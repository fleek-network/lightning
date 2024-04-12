use core::mem;

use aya_bpf::bindings::xdp_action;
use aya_bpf::macros::xdp;
use aya_bpf::programs::XdpContext;
use aya_log_ebpf::info;
use common::{PacketFilter, PacketFilterParams};
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

    if let Some(params) = try_match_only_ip(ip) {
        return Ok(params.action);
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

    if let Some(params) = try_match(PacketFilter {
        ip,
        port,
        proto: proto as u16,
    }) {
        return Ok(params.action);
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

fn try_match(filter: PacketFilter) -> Option<PacketFilterParams> {
    unsafe {
        // Try a specific match.
        let mut result = maps::PACKET_FILTERS.get(&filter).copied();

        // Try for any port.
        if result.is_none() {
            result = maps::PACKET_FILTERS
                .get(&PacketFilter {
                    ip: filter.ip,
                    port: 0,
                    proto: filter.proto,
                })
                .copied();
        }

        // Try for any protocol.
        if result.is_none() {
            result = maps::PACKET_FILTERS
                .get(&PacketFilter {
                    ip: filter.ip,
                    port: filter.port,
                    proto: u16::MAX,
                })
                .copied()
        }

        result
    }
}

fn try_match_only_ip(ip: u32) -> Option<PacketFilterParams> {
    unsafe {
        maps::PACKET_FILTERS
            .get(&PacketFilter {
                ip,
                port: 0,
                proto: u16::MAX,
            })
            .copied()
    }
}
