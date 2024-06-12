use core::ffi::c_char;

use aya_ebpf::macros::lsm;
use aya_ebpf::programs::LsmContext;
use lightning_ebpf_common::{File, Profile, MAX_FILE_RULES};

use crate::{access, maps, vmlinux};

pub const ALLOW: i32 = 0;
pub const DENY: i32 = -1;

#[lsm(hook = "file_open")]
pub fn file_open(ctx: LsmContext) -> i32 {
    unsafe { try_file_open(ctx).unwrap_or_else(|e| e) }
}

unsafe fn try_file_open(ctx: LsmContext) -> Result<i32, i32> {
    let task_inode = get_inode_from_current_task().map_err(|_| ALLOW)?;

    if let Some(profile) = maps::PROFILES.get(&File::new(task_inode)) {
        let file: *const vmlinux::file = ctx.arg(0);

        // Todo: add cache.
        let _inode = {
            let inode = aya_ebpf::helpers::bpf_probe_read_kernel(access::file_inode(file))
                .map_err(|_| DENY)?;
            aya_ebpf::helpers::bpf_probe_read_kernel(access::inode_i_ino(inode))
                .map_err(|_| DENY)?
        };

        // Todo: Move this buffer into a map so we can deal with pointer
        // and remove this from the stack.
        let mut path = [0u8; 64];
        let btf_path = access::file_f_path(file);
        if aya_ebpf::helpers::bpf_d_path(
            btf_path as *mut _,
            path.as_mut_ptr() as *mut c_char,
            path.len() as u32,
        ) < 0
        {
            // It's possible that the buffer did not have enough capacity.
            // Todo: send event?
            return Err(DENY);
        }

        return verify_access(&ctx, path.as_slice(), profile);
    }

    Ok(ALLOW)
}

fn verify_access(_ctx: &LsmContext, target_path: &[u8], profile: &Profile) -> Result<i32, i32> {
    for i in 0..MAX_FILE_RULES {
        let rule = profile.rules.get(i).ok_or(-1)?;
        let full_path = read_bytes(64i64, rule.path.as_slice())?;
        let mut found = true;
        for j in 0..64 {
            if full_path[j] != target_path[j] {
                found = false;
            }
            if target_path[j] == 0 && found {
                return Ok(ALLOW);
            }
        }
    }

    Ok(DENY)
}

/// Performs slicing with proper bound checks.
fn read_bytes(len: i64, dest: &[u8]) -> Result<&[u8], i32> {
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
