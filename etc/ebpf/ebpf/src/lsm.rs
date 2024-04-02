use aya_bpf::cty::c_int;
use aya_bpf::macros::{lsm, map};
use aya_bpf::maps::HashMap;
use aya_bpf::programs::LsmContext;
use aya_log_ebpf::info;
use common::File;

use crate::vmlinux;
use crate::vmlinux::{cred, task_struct};

// Todo: replace with HashMapOfMaps or HashMapOfArrays when aya adds support.
#[map]
static FILE_OPEN_ALLOWED: HashMap<u64, u32> = HashMap::<u64, u32>::with_max_entries(1024, 0);
#[map]
static BINARY_FILES: HashMap<File, u64> = HashMap::<File, u64>::with_max_entries(1024, 0);
#[map]
static PROCESSES: HashMap<i32, u64> = HashMap::<i32, u64>::with_max_entries(1024, 0);

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
    let pid = aya_bpf::helpers::bpf_get_current_pid_tgid();
    let _file: *const vmlinux::file = ctx.arg(0);
    info!(&ctx, "Process {} attempting to open file", pid);
    Ok(0)
}
