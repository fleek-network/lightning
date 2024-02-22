#![no_std]
#![no_main]

use core::mem;

use aya_bpf::bindings::xdp_action;
use aya_bpf::macros::xdp;
use aya_bpf::programs::XdpContext;
use aya_log_ebpf::info;
use memoffset::offset_of;
use network_types::eth::{EthHdr, EtherType};
use network_types::ip::Ipv4Hdr;

#[xdp]
pub fn xdp_packet_filter(ctx: XdpContext) -> u32 {
    match unsafe { filter(ctx) } {
        Ok(ret) => ret,
        // Todo: let's handle this differently.
        Err(_) => xdp_action::XDP_ABORTED,
    }
}

unsafe fn filter(ctx: XdpContext) -> Result<u32, ()> {
    let h_proto = unsafe { *ptr_at::<EtherType>(&ctx, offset_of!(EthHdr, ether_type))? };
    if h_proto != EtherType::Ipv4 {
        return Ok(xdp_action::XDP_PASS);
    }
    let source =
        u32::from_be_bytes(unsafe { *ptr_at(&ctx, EthHdr::LEN + offset_of!(Ipv4Hdr, src_addr))? });
    info!(&ctx, "received a packet from {:i}", source);
    Ok(xdp_action::XDP_PASS)
}

fn ptr_at<T>(ctx: &XdpContext, offset: usize) -> Result<*const T, ()> {
    let start = ctx.data();
    let end = ctx.data_end();
    let len = mem::size_of::<T>();

    if start + offset + len > end {
        return Err(());
    }

    Ok((start + offset) as *const T)
}

#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    unsafe { core::hint::unreachable_unchecked() }
}
