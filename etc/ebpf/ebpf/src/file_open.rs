use aya_bpf::macros::lsm;
use aya_bpf::programs::LsmContext;
use aya_log_ebpf::info;
use common::File;

use crate::{maps, vmlinux};

#[lsm(hook = "file_open")]
pub fn file_open(ctx: LsmContext) -> i32 {
    unsafe { try_file_open(ctx).unwrap_or_else(|ret| ret) }
}

unsafe fn try_file_open(ctx: LsmContext) -> Result<i32, i32> {
    let kfile: *const vmlinux::file = ctx.arg(0);
    // Todo: why does this helper return a long?
    let inode = aya_bpf::helpers::bpf_probe_read_kernel(&(*kfile).f_inode).map_err(|_| -1i32)?;
    let inode_n = aya_bpf::helpers::bpf_probe_read_kernel(&(*inode).i_ino).map_err(|_| -1i32)?;
    // Todo: Get device for regular and special files.
    let file = File {
        inode_n,
        dev: 0,
        rdev: 0,
    };
    match validate_by_pid(&ctx, &file) {
        true => Ok(0),
        false => Ok(-1),
    }
}

unsafe fn validate_by_pid(ctx: &LsmContext, file: &File) -> bool {
    let pid = aya_bpf::helpers::bpf_get_current_pid_tgid();
    info!(ctx, "Process {} attempting to open file", pid);
    if let Some(f_inode) = maps::PROCESSES_TO_FILE.get(&pid) {
        return f_inode == &file.inode_n;
    }
    // Todo: get binary path otherwise.
    false
}
