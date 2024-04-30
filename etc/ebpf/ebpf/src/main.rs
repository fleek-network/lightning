#![no_std]
#![no_main]

mod access;
mod file_open;
mod maps;
mod packet_filter;
mod task_alloc;
mod task_fix_setuid;
#[allow(non_camel_case_types)]
#[allow(non_upper_case_globals)]
#[allow(non_snake_case)]
#[allow(dead_code)]
mod vmlinux;

#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    unsafe { core::hint::unreachable_unchecked() }
}
