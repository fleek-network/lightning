use core::ffi::c_char;

use aya_ebpf::macros::lsm;
use aya_ebpf::programs::LsmContext;
use aya_log_ebpf::info;
use lightning_ebpf_common::File;

use crate::{access, maps, vmlinux};

pub const ALLOW: i32 = 0;
pub const DENY: i64 = -1;

#[lsm(hook = "file_open")]
pub fn file_open(ctx: LsmContext) -> i32 {
    unsafe { try_file_open(ctx).unwrap_or_else(|_| ALLOW) }
}

unsafe fn try_file_open(ctx: LsmContext) -> Result<i32, i64> {
    let task_inode = get_inode_from_current_task()?;
    if maps::FILE_RULES.get(&File::new(task_inode)).is_some() {
        let target_file: *const vmlinux::file = ctx.arg(0);
        let _target_inode = {
            let inode = aya_ebpf::helpers::bpf_probe_read_kernel(access::file_inode(target_file))?;
            aya_ebpf::helpers::bpf_probe_read_kernel(access::inode_i_ino(inode))?
        };

        // Todo: add cache.

        // Todo: Move this buffer into a map so we can deal with pointer and remove this from the
        let mut buf = [0u8; 64];
        let btf_path = access::file_f_path(target_file);
        if aya_ebpf::helpers::bpf_d_path(
            btf_path as *mut _,
            buf.as_mut_ptr() as *mut c_char,
            buf.len() as u32,
        ) < 0
        {
            // It's possible that the buffer did not have enough capacity.
            // Todo: log event.
            return Err(DENY);
        }

        let target_path = "/home/mmeier/Code/notebook/logs.txt".as_bytes();
        let full_path = read_bytes(target_path.len() as i64, buf.as_slice()).map_err(|_| -1)?;
        if target_path == full_path {
            info!(&ctx, "It's a match");
        } else {
            info!(&ctx, "Not a match");
        }
    }

    Ok(0)
}

/// Performs slicing with proper bound checks.
fn read_bytes(len: i64, dest: &[u8]) -> Result<&[u8], i64> {
    if !aya_ebpf::check_bounds_signed(len, 0, dest.len() as i64) {
        return Err(DENY);
    }

    let len = usize::try_from(len).map_err(|_| DENY)?;
    dest.get(..len).ok_or(-1)
}

/// Get the inode number of the current process's binary file.
unsafe fn get_inode_from_current_task() -> Result<u64, i64> {
    let task = aya_ebpf::helpers::bpf_get_current_task() as *mut vmlinux::task_struct;
    let mm = aya_ebpf::helpers::bpf_probe_read_kernel(access::task_struct_mm(task))?;
    let file = aya_ebpf::helpers::bpf_probe_read_kernel(access::mm_exe_file(mm))?;
    let f_inode = aya_ebpf::helpers::bpf_probe_read_kernel(access::file_inode(file))?;
    aya_ebpf::helpers::bpf_probe_read_kernel(access::inode_i_ino(f_inode))
}
