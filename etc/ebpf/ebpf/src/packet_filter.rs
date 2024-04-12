use core::mem;

use aya_bpf::bindings::xdp_action;
use aya_bpf::macros::xdp;
use aya_bpf::programs::XdpContext;
use aya_log_ebpf::info;
use common::PacketFilter;
use memoffset::offset_of;
use network_types::eth::{EthHdr, EtherType};
use network_types::ip::{IpProto, Ipv4Hdr};
use network_types::tcp::TcpHdr;
use network_types::udp::UdpHdr;

use crate::maps;

type XdpAction = xdp_action::Type;

#[xdp]
pub fn xdp_packet_filter(ctx: XdpContext) -> u32 {
    match unsafe { filter(ctx) } {
        Ok(ret) => ret,
        Err(_) => xdp_action::XDP_ABORTED,
    }
}

unsafe fn filter(ctx: XdpContext) -> Result<u32, ()> {
    let h_proto = unsafe { *ptr_at::<EtherType>(&ctx, offset_of!(EthHdr, ether_type))? };
    match h_proto {
        EtherType::Ipv4 => process_ipv4(&ctx),
        _ => Ok(xdp_action::XDP_PASS),
    }
}

fn process_ipv4(ctx: &XdpContext) -> Result<XdpAction, ()> {
    let ip =
        u32::from_be_bytes(unsafe { *ptr_at(&ctx, EthHdr::LEN + offset_of!(Ipv4Hdr, src_addr))? });

    info!(ctx, "received a packet from {:i}", ip);

    let filter = match get_filter(ip) {
        Some(f) => f,
        None => return Ok(xdp_action::XDP_PASS),
    };

    // Handle wild cards.
    if filter.port == 0u16 && filter.proto == 0u16 {
        return Ok(filter.action);
    }

    let proto = unsafe { *ptr_at::<IpProto>(&ctx, EthHdr::LEN + offset_of!(Ipv4Hdr, proto))? };
    let port = match proto {
        IpProto::Tcp => u16::from_be_bytes(unsafe {
            *ptr_at(&ctx, EthHdr::LEN + Ipv4Hdr::LEN + offset_of!(TcpHdr, dest))?
        }),
        IpProto::Udp => u16::from_be_bytes(unsafe {
            *ptr_at(&ctx, EthHdr::LEN + Ipv4Hdr::LEN + offset_of!(UdpHdr, dest))?
        }),
        _ => {
            return Ok(xdp_action::XDP_PASS);
        },
    };

    let proto = proto as u16;
    if (port == filter.port && proto == filter.proto)
        || (port == 0u16 && proto == filter.proto)
        || (proto == 0u16 && port == filter.port)
    {
        return Ok(filter.action);
    }

    Ok(xdp_action::XDP_PASS)
}

// Before any data access, the verifier requires us to do a bound check.
fn ptr_at<T>(ctx: &XdpContext, offset: usize) -> Result<*const T, ()> {
    let start = ctx.data();
    let end = ctx.data_end();
    let len = mem::size_of::<T>();

    if start + offset + len > end {
        return Err(());
    }

    Ok((start + offset) as *const T)
}

fn get_filter(ip: u32) -> Option<PacketFilter> {
    unsafe { maps::PACKET_FILTERS.get(&ip).copied() }
}
