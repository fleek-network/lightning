use aya_bpf::cty::c_int;
use aya_bpf::macros::lsm;
use aya_bpf::programs::LsmContext;
use aya_log_ebpf::info;

use crate::vmlinux;
use crate::vmlinux::{cred, task_struct};

const PATH_LEN: usize = 64;

#[lsm(hook = "task_alloc")]
pub fn task_alloc(ctx: LsmContext) -> i32 {
    unsafe { try_task_alloc(ctx).unwrap_or_else(|ret| ret) }
}

unsafe fn try_task_alloc(ctx: LsmContext) -> Result<i32, i32> {
    let task: *const task_struct = ctx.arg(0);
    let pid: c_int = (*task).pid;
    info!(&ctx, "Process with PID {} spawned a child process", pid);
    Ok(0)
}

#[lsm(hook = "task_fix_setuid")]
pub fn task_fix_setuid(ctx: LsmContext) -> i32 {
    unsafe { try_task_fix_setuid(ctx).unwrap_or_else(|ret| ret) }
}

unsafe fn try_task_fix_setuid(ctx: LsmContext) -> Result<i32, i32> {
    let new: *const cred = ctx.arg(0);
    let new_uid = (*new).uid.val;
    let old: *const cred = ctx.arg(1);
    let old_uid = (*old).uid.val;
    if new_uid == 0 && old_uid != 0 {
        info!(&ctx, "User {} is attempting to log in as root", old_uid);
    } else {
        info!(&ctx, "User {} changed its uid to {}", old_uid, new_uid);
    }
    Ok(0)
}

#[lsm(hook = "file_open")]
pub fn file_open(ctx: LsmContext) -> i32 {
    unsafe { try_file_open(ctx).unwrap_or_else(|ret| ret) }
}

unsafe fn try_file_open(ctx: LsmContext) -> Result<i32, i32> {
    let buf = [0u8; PATH_LEN];
    let path = {
        let _file: *const vmlinux::file = ctx.arg(0);
        let len = {
            // Todo: We need to use the file's inode or dentry
            // to find the parent directories and validate the access.
            PATH_LEN
        };
        if len >= PATH_LEN {
            // Todo: revisit.
            return Err(0);
        }
        // Todo: revisit.
        core::str::from_utf8(&buf[..len]).map_err(|_| -1)?
    };

    // Todo: add other directories.
    if path.starts_with("/proc/") || path.starts_with("/sys/") || path.contains(".lightning") {
        return Err(-1);
    }

    Ok(0)
}
