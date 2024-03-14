#![no_std]
#![no_main]

use aya_bpf::bindings;
use aya_bpf::cty::c_char;
use aya_bpf::macros::lsm;
use aya_bpf::programs::LsmContext;
use ebpf::vmlinux;

const PATH_LEN: usize = 64;

unsafe fn get_path(file: *const vmlinux::file, buf: &mut [u8]) -> Result<usize, i64> {
    // Todo: get path from inode.
    todo!()
}

#[lsm(hook = "file_open")]
pub fn file_open(ctx: LsmContext) -> i32 {
    try_file_open(ctx).unwrap_or_else(|ret| ret)
}

fn try_file_open(ctx: LsmContext) -> Result<i32, i32> {
    let mut buf = [0u8; PATH_LEN];
    let path = unsafe {
        let file: *const vmlinux::file = ctx.arg(0);
        let len = get_path(file, &mut buf).map_err(|_| 0)?;
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
