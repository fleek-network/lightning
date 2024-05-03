use aya_ebpf::macros::lsm;
use aya_ebpf::programs::LsmContext;
use aya_log_ebpf::info;

use crate::vmlinux::cred;

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
