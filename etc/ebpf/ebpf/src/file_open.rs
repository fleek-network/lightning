use core::ffi::c_char;

use aya_ebpf::macros::lsm;
use aya_ebpf::programs::LsmContext;
use lightning_ebpf_common::{
    File,
    FileCacheKey,
    FileRule,
    GlobalConfig,
    Profile,
    ACCESS_DENIED_EVENT,
    EVENT_HEADER_SIZE,
    FILE_OPEN_PROG_ID,
    MAX_BUFFER_LEN,
    MAX_FILE_RULES,
};

use crate::{access, maps, vmlinux};

pub const ALLOW: i32 = 0;
pub const DENY: i32 = -1;

#[lsm(hook = "file_open")]
pub fn file_open(ctx: LsmContext) -> i32 {
    unsafe { try_file_open(ctx).unwrap_or_else(|e| e) }
}

unsafe fn try_file_open(ctx: LsmContext) -> Result<i32, i32> {
    let file: *const vmlinux::file = ctx.arg(0);
    let task_file = get_file_from_current_task().map_err(|_| ALLOW)?;
    let task_inode = read_file_inode(task_file).map_err(|_| ALLOW)?;

    if let Some(profile) = maps::PROFILES.get(&File::new(task_inode)) {
        let global_config = maps::GLOBAL_CONFIG.get(&0).ok_or(DENY)?;

        // Get the path for the target file.
        let buf = maps::BUFFERS.get_ptr_mut(&0).ok_or(DENY)?;
        let path = buf.as_mut().ok_or(DENY)?.as_mut_slice();
        read_path(file, path)?;

        if global_config.mode == GlobalConfig::ENFORCE_MODE {
            // Get the inode of the target file.
            let target_inode = {
                let inode = aya_ebpf::helpers::bpf_probe_read_kernel(access::file_inode(file))
                    .map_err(|_| DENY)?;
                aya_ebpf::helpers::bpf_probe_read_kernel(access::inode_i_ino(inode))
                    .map_err(|_| DENY)?
            };

            // Check the cache first.
            let cache_key = &FileCacheKey {
                target: target_inode,
                task: task_inode,
            };
            if let Some(cached_rule) = maps::FILE_CACHE.get(&cache_key) {
                if (cached_rule.is_dir == FileRule::IS_DIR
                    && cmp_str_bytes(cached_rule.path.as_slice(), path, MAX_BUFFER_LEN))
                    || cmp_str_bytes(path, cached_rule.path.as_slice(), MAX_BUFFER_LEN)
                        && cached_rule.permissions & FileRule::OPEN_MASK > 0
                {
                    return Ok(ALLOW);
                }
                // If we get here, it means that the file's inode number
                // has been assigned to a different file.
            }

            // Go through the rules in this profile and try to find a match.
            if let Some(rule) = find_match(profile, path, FileRule::OPEN_MASK)? {
                let _ = maps::FILE_CACHE.insert(cache_key, rule, 0);
                return Ok(ALLOW);
            }
        }

        // The error may indicate that the ring buffer is full so there is nothing
        // left to do but let the consumer catch up.
        let _ = send_event(ACCESS_DENIED_EVENT, FILE_OPEN_PROG_ID, task_file, path);

        return if global_config.mode == GlobalConfig::LEARN_MODE {
            Ok(ALLOW)
        } else {
            Ok(DENY)
        };
    }

    Ok(ALLOW)
}

/// Read the file's path into dst.
unsafe fn read_path(file: *const vmlinux::file, dst: &mut [u8]) -> Result<i32, i32> {
    let btf_path = access::file_f_path(file);
    if aya_ebpf::helpers::bpf_d_path(
        btf_path as *mut _,
        dst.as_mut_ptr() as *mut c_char,
        dst.len() as u32,
    ) < 0
    {
        // It's possible that the buffer did not have enough capacity.
        return Err(DENY);
    }
    Ok(ALLOW)
}

/// Tries to find a rule that matches the target path and mask.
fn find_match<'a>(
    profile: &'a Profile,
    target_path: &[u8],
    mask: u32,
) -> Result<Option<&'a FileRule>, i32> {
    for i in 0..MAX_FILE_RULES {
        let rule = profile.rules.get(i).ok_or(DENY)?;

        if rule.permissions & mask == 0 {
            continue;
        }

        let full_path = rule.path.as_slice();
        if (rule.is_dir == FileRule::IS_FILE
            && cmp_str_bytes(full_path, target_path, MAX_BUFFER_LEN))
            || cmp_str_bytes(target_path, full_path, MAX_BUFFER_LEN)
        {
            return Ok(Some(rule));
        }
    }

    Ok(None)
}

/// Get the current process's binary file.
unsafe fn get_file_from_current_task() -> Result<*const vmlinux::file, i64> {
    let task = aya_ebpf::helpers::bpf_get_current_task() as *mut vmlinux::task_struct;
    let mm = aya_ebpf::helpers::bpf_probe_read_kernel(access::task_struct_mm(task))?;
    aya_ebpf::helpers::bpf_probe_read_kernel(access::mm_exe_file(mm))
}

/// Read the inode number from the kernel file struct.
unsafe fn read_file_inode(file: *const vmlinux::file) -> Result<u64, i64> {
    let f_inode = aya_ebpf::helpers::bpf_probe_read_kernel(access::file_inode(file))?;
    aya_ebpf::helpers::bpf_probe_read_kernel(access::inode_i_ino(f_inode))
}

/// Read the file name from the kernel file struct.
unsafe fn read_file_name(file: *const vmlinux::file, dst: &mut [u8]) -> Result<&[u8], i64> {
    let dentry = aya_ebpf::helpers::bpf_probe_read_kernel(access::file_dentry(file))?;
    let d_name = aya_ebpf::helpers::bpf_probe_read_kernel(access::dentry_d_name_name(dentry))?;
    aya_ebpf::helpers::bpf_probe_read_kernel_str_bytes(d_name, dst)
}

// The eBPF verifier may reject this code
// if `size` is not equal to the len of both parameters.
/// Compares slices to determine if they both contain the same NUL terminated string.
fn cmp_str_bytes(a: &[u8], b: &[u8], size: usize) -> bool {
    for j in 0..size {
        if a[j] != b[j] {
            break;
        }

        if a[j] == 0 {
            return true;
        }
    }
    false
}

/// Send an event to userspace.
unsafe fn send_event(event: u8, prog: u8, task: *const vmlinux::file, message: &[u8]) {
    let Some(buf) = maps::BUFFERS
        .get_ptr_mut(&1)
        .map(|buf| buf.as_mut())
        .flatten()
    else {
        return;
    };

    let Some(task_name) = read_file_name(task, buf.as_mut_slice()).ok() else {
        return;
    };

    if let Some(mut entry) =
        maps::EVENTS.reserve::<[u8; MAX_BUFFER_LEN + MAX_BUFFER_LEN + EVENT_HEADER_SIZE]>(0)
    {
        match entry.as_mut_ptr().as_mut() {
            Some(dst) => {
                // Set headers.
                dst[0] = event;
                dst[1] = prog;

                // Set messages.
                if aya_ebpf::helpers::bpf_probe_read_kernel_str_bytes(
                    task_name.as_ptr(),
                    core::slice::from_raw_parts_mut(
                        dst.as_mut_ptr().byte_add(EVENT_HEADER_SIZE),
                        MAX_BUFFER_LEN,
                    ),
                )
                .is_err()
                {
                    entry.discard(0);
                    return;
                }

                if aya_ebpf::helpers::bpf_probe_read_kernel_str_bytes(
                    message.as_ptr(),
                    core::slice::from_raw_parts_mut(
                        dst.as_mut_ptr()
                            .byte_add(EVENT_HEADER_SIZE + MAX_BUFFER_LEN),
                        MAX_BUFFER_LEN,
                    ),
                )
                .is_ok()
                {
                    entry.submit(0);
                } else {
                    entry.discard(0);
                }
            },
            None => {
                entry.discard(0);
            },
        }
    };
}
