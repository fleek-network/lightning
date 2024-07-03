use core::ffi::c_char;

use lightning_ebpf_common::{FileRule, Profile, EVENT_HEADER_SIZE, MAX_BUFFER_LEN, MAX_FILE_RULES};

use crate::{access, maps, vmlinux};

pub const ALLOW: i32 = 0;
pub const DENY: i32 = -1;

/// Read the file's path into dst.
pub unsafe fn read_path(file: *const vmlinux::file, dst: &mut [u8]) -> Result<i32, i32> {
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
pub fn find_match<'a>(
    profile: &'a Profile,
    target_path: &[u8],
    mask: u32,
) -> Result<Option<&'a FileRule>, i32> {
    for i in 0..MAX_FILE_RULES {
        let rule = profile.rules.get(i).ok_or(DENY)?;

        if rule.permissions & mask == 0 {
            continue;
        }

        if contains(rule.path.as_slice(), target_path, MAX_BUFFER_LEN) {
            return Ok(Some(rule));
        }
    }

    Ok(None)
}

/// Get the current process's binary file.
pub unsafe fn get_file_from_current_task() -> Result<*const vmlinux::file, i64> {
    let task = aya_ebpf::helpers::bpf_get_current_task() as *mut vmlinux::task_struct;
    let mm = aya_ebpf::helpers::bpf_probe_read_kernel(access::task_struct_mm(task))?;
    aya_ebpf::helpers::bpf_probe_read_kernel(access::mm_exe_file(mm))
}

/// Read the inode number from the kernel file struct.
pub unsafe fn read_file_inode(file: *const vmlinux::file) -> Result<u64, i64> {
    let f_inode = aya_ebpf::helpers::bpf_probe_read_kernel(access::file_inode(file))?;
    aya_ebpf::helpers::bpf_probe_read_kernel(access::inode_i_ino(f_inode))
}

/// Read the file name from the kernel file struct.
unsafe fn read_file_name(file: *const vmlinux::file, dst: &mut [u8]) -> Result<&[u8], i64> {
    let dentry = aya_ebpf::helpers::bpf_probe_read_kernel(access::file_dentry(file))?;
    let d_name = aya_ebpf::helpers::bpf_probe_read_kernel(access::dentry_d_name_name(dentry))?;
    aya_ebpf::helpers::bpf_probe_read_kernel_str_bytes(d_name, dst)
}

// The eBPF verifier may reject this code if:
//  - `size` is not equal to the len of both parameters.
//  - `size` is zero.
// Due to limited stack space, we do not handle these cases in this function.
/// Returns true if the given pattern matches a sub-slice of the target string slice.
pub fn contains(pat: &[u8], target: &[u8], size: usize) -> bool {
    // Handle empty strings.
    if pat[0] == 0 {
        return target[0] == 0;
    }

    for i in 0..size {
        if pat[i] == 0 {
            return true;
        }

        if pat[i] != target[i] {
            break;
        }
    }

    false
}

/// Send an event to userspace.
pub unsafe fn send_event(event: u8, prog: u8, task: *const vmlinux::file, message: &[u8]) {
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
