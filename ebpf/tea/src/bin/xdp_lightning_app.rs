use anyhow::Context;
use aya::programs::{Xdp, XdpFlags};
use aya::{include_bytes_aligned, Bpf};
use aya_log::BpfLogger;
use tokio::signal;

#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    env_logger::init();

    #[cfg(debug_assertions)]
    let mut handle = Bpf::load(include_bytes_aligned!(
        "../../../ebpf/target/bpfel-unknown-none/debug/packet_filter"
    ))?;
    #[cfg(not(debug_assertions))]
    let mut bpf = Bpf::load(include_bytes_aligned!(
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
        .attach("enp0s31f6", XdpFlags::default())
        .context("failed to attach the XDP program")?;

    log::info!("Enter Ctrl-C to shutdown");
    signal::ctrl_c().await?;

    Ok(())
}
