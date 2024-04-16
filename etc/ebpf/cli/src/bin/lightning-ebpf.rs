use anyhow::Context;
use aya::maps::HashMap;
use aya::programs::{Xdp, XdpFlags};
use aya::{include_bytes_aligned, Bpf};
use aya_log::BpfLogger;
use clap::Parser;
use common::{File, FileRuleList, PacketFilter, PacketFilterParams};
use ebpf_service::map::SharedMap;
use ebpf_service::server::Server;
use tokio::net::UnixListener;
use tokio::signal;

#[derive(Debug, Parser)]
struct Opts {
    /// Interface to attach packet filter program.
    #[clap(short, long, default_value = "eth0")]
    iface: String,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let opt = Opts::parse();

    env_logger::init();

    #[cfg(debug_assertions)]
    let mut handle = Bpf::load(include_bytes_aligned!(
        "../../../ebpf/target/bpfel-unknown-none/debug/ebpf"
    ))?;
    #[cfg(not(debug_assertions))]
    let mut handle = Bpf::load(include_bytes_aligned!(
        "../../../ebpf/target/bpfel-unknown-none/release/ebpf"
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

    let packet_filters: HashMap<_, PacketFilter, PacketFilterParams> =
        HashMap::try_from(handle.take_map("PACKET_FILTERS").unwrap())?;
    let file_open_allow: HashMap<_, File, FileRuleList> =
        HashMap::try_from(handle.take_map("FILE_OPEN_ALLOW").unwrap())?;

    let listener = UnixListener::bind(".lightning/ebpf")?;
    let shared_state = SharedMap::new(packet_filters, file_open_allow);
    let server = Server::new(listener, shared_state);

    log::info!("Enter Ctrl-C to shutdown");

    tokio::select! {
        _ = server.start() => {}
        _ = signal::ctrl_c() => {}
    }

    Ok(())
}
