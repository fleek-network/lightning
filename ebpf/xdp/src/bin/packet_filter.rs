#![no_std]
#![no_main]

use aya_bpf::bindings::xdp_action;
use aya_bpf::macros::xdp;
use aya_bpf::programs::XdpContext;
use aya_log_ebpf::info;

#[xdp]
pub fn xdp_packet_filter(ctx: XdpContext) -> u32 {
    match unsafe { filter(ctx) } {
        Ok(ret) => ret,
        Err(_) => xdp_action::XDP_ABORTED,
    }
}

unsafe fn filter(ctx: XdpContext) -> Result<u32, u32> {
    info!(&ctx, "received a packet");
    Ok(xdp_action::XDP_PASS)
}

#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    unsafe { core::hint::unreachable_unchecked() }
}
