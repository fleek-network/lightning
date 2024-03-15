// Monitoring and Mandatory Access Control using LSM.
#![no_std]
#![no_main]

use aya_bpf::bindings;
use aya_bpf::cty::{c_char, c_int};
use aya_bpf::macros::lsm;
use aya_bpf::programs::LsmContext;
use aya_log_ebpf::info;
use ebpf::vmlinux;
use ebpf::vmlinux::task_struct;

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

#[lsm(hook = "file_open")]
pub fn file_open(ctx: LsmContext) -> i32 {
    unsafe { try_file_open(ctx).unwrap_or_else(|ret| ret) }
}

unsafe fn try_file_open(ctx: LsmContext) -> Result<i32, i32> {
    let mut buf = [0u8; PATH_LEN];
    let path = {
        let file: *const vmlinux::file = ctx.arg(0);
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

#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    unsafe { core::hint::unreachable_unchecked() }
}
