use std::net::Ipv4Addr;

use anyhow::Context;
use aya::maps::HashMap;
use aya::programs::{Xdp, XdpFlags};
use aya::{include_bytes_aligned, Bpf};
use aya_log::BpfLogger;
use clap::Parser;
use tokio::signal;

#[derive(Debug, Parser)]
struct Opts {
    /// Interface to attach xdp program to.
    #[clap(short, long, default_value = "eth0")]
    iface: String,
    /// Ip address to block.
    #[clap(short, long)]
    block: Option<Ipv4Addr>,
}

#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    let opt = Opts::parse();

    env_logger::init();

    #[cfg(debug_assertions)]
    let mut handle = Bpf::load(include_bytes_aligned!(
        "../../../xdp/target/bpfel-unknown-none/debug/packet_filter"
    ))?;
    #[cfg(not(debug_assertions))]
    let mut handle = Bpf::load(include_bytes_aligned!(
        "../../../xdp/target/bpfel-unknown-none/release/packet_filter"
    ))?;

    if let Err(e) = BpfLogger::init(&mut handle) {
        log::warn!("failed to initialize logger: {}", e);
    }

    let program: &mut Xdp = handle
        .program_mut("xdp_packet_filter")
        .unwrap()
        .try_into()?;
    program.load()?;
    program
        .attach(&opt.iface, XdpFlags::default())
        .context("failed to attach the XDP program")?;

    let mut blocklist: HashMap<_, u32, u32> =
        HashMap::try_from(handle.map_mut("BLOCK_LIST").unwrap())?;

    if let Some(block_addr) = opt.block {
        let addr: u32 = block_addr.try_into()?;
        blocklist.insert(addr, 0, 0)?;
    }

    log::info!("Enter Ctrl-C to shutdown");
    signal::ctrl_c().await?;

    Ok(())
}
