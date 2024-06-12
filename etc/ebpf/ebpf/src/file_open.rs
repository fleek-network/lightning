use core::ffi::c_char;

use aya_ebpf::macros::lsm;
use aya_ebpf::programs::LsmContext;
use lightning_ebpf_common::{
    Buffer,
    File,
    FileCacheKey,
    FileRule,
    Profile,
    MAX_FILE_RULES,
    MAX_PATH_LEN,
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
    let task_inode = get_inode_from_current_task().map_err(|_| ALLOW)?;

    if let Some(profile) = maps::PROFILES.get(&File::new(task_inode)) {
        // Get the path for the target file.
        let buf = maps::BUFFERS
            .get_ptr_mut(&Buffer::OPEN_FILE_BUFFER)
            .ok_or(DENY)?;
        let path = buf.as_mut().ok_or(DENY)?.buffer.as_mut();
        read_path(file, path)?;

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
            if cmp_slices(path, cached_rule.path.as_slice(), MAX_PATH_LEN) {
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

        return Ok(DENY);
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
        if cmp_slices(target_path, full_path, MAX_PATH_LEN) {
            return Ok(Some(rule));
        }
    }

    Ok(None)
}

/// Get the inode number of the current process's binary file.
unsafe fn get_inode_from_current_task() -> Result<u64, i64> {
    let task = aya_ebpf::helpers::bpf_get_current_task() as *mut vmlinux::task_struct;
    let mm = aya_ebpf::helpers::bpf_probe_read_kernel(access::task_struct_mm(task))?;
    let file = aya_ebpf::helpers::bpf_probe_read_kernel(access::mm_exe_file(mm))?;
    let f_inode = aya_ebpf::helpers::bpf_probe_read_kernel(access::file_inode(file))?;
    aya_ebpf::helpers::bpf_probe_read_kernel(access::inode_i_ino(f_inode))
}

// The eBPF verifier may reject this code
// if `size` is not equal to the len of both parameters.
fn cmp_slices(a: &[u8], b: &[u8], size: usize) -> bool {
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
