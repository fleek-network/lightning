use std::path::PathBuf;

use anyhow::Context;
use aya::maps::{HashMap, PerCpuHashMap, PerCpuValues};
use aya::programs::{Lsm, Xdp, XdpFlags};
use aya::{include_bytes_aligned, Btf, Ebpf};
use aya_log::EbpfLogger;
use clap::Parser;
use lightning_ebpf_common::{
    Buffer,
    File,
    PacketFilter,
    PacketFilterParams,
    Profile,
    MAX_BUFFER_LEN,
};
use lightning_ebpf_service::map::SharedMap;
use lightning_ebpf_service::server::Server;
use lightning_ebpf_service::{ConfigSource, PathConfig};
use tokio::net::UnixListener;
use tokio::signal;

#[derive(Debug, Parser)]
struct Opts {
    /// Interface to attach packet filter program.
    #[clap(short, long)]
    iface: String,
    /// Path to packet filter config.
    #[clap(short, long)]
    pf: PathBuf,
    /// Path to temporary directory.
    #[clap(short, long)]
    tmp: PathBuf,
    /// Path to profile directory.
    #[clap(short = 's', long)]
    profile: PathBuf,
    /// Bind path.
    #[clap(short, long)]
    bind: PathBuf,
    /// Load FILE_OPEN program in the corresponding LSM hook.
    #[clap(short, long, default_value_t = true)]
    enable_file_open: bool,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let opt = Opts::parse();

    env_logger::init();

    #[cfg(debug_assertions)]
    let mut handle = Ebpf::load(include_bytes_aligned!(
        "../../ebpf/target/bpfel-unknown-none/debug/lightning-ebpf-programs"
    ))?;
    #[cfg(not(debug_assertions))]
    let mut handle = Ebpf::load(include_bytes_aligned!(
        "../../ebpf/target/bpfel-unknown-none/release/lightning-ebpf-programs"
    ))?;

    if let Err(e) = EbpfLogger::init(&mut handle) {
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

    let mut _file_open_prog: Option<&mut Lsm> = None;
    if opt.enable_file_open {
        let prog: &mut Lsm = handle.program_mut("file_open").unwrap().try_into()?;
        let btf = Btf::from_sys_fs()?;
        prog.load("file_open", &btf)?;
        prog.attach()?;
        _file_open_prog = Some(prog);
    }

    let file_open_allow: HashMap<_, File, Profile> =
        HashMap::try_from(handle.take_map("PROFILES").unwrap())?;
    let packet_filters: HashMap<_, PacketFilter, PacketFilterParams> =
        HashMap::try_from(handle.take_map("PACKET_FILTERS").unwrap())?;
    let mut buffers: PerCpuHashMap<_, u32, Buffer> =
        PerCpuHashMap::try_from(handle.take_map("BUFFERS").unwrap())?;

    buffers.insert(
        1,
        PerCpuValues::try_from(vec![[0u8; MAX_BUFFER_LEN]; aya::util::nr_cpus()?])?,
        0,
    )?;
    buffers.insert(
        1,
        PerCpuValues::try_from(vec![[0u8; MAX_BUFFER_LEN]; aya::util::nr_cpus()?])?,
        0,
    )?;

    let path_config = PathConfig {
        tmp_dir: opt.tmp,
        packet_filter: opt.pf,
        profiles_dir: opt.profile,
    };
    let config_src = ConfigSource::new(path_config);

    let _ = tokio::fs::remove_file(opt.bind.as_path()).await;
    let listener = UnixListener::bind(opt.bind.as_path())?;

    let shared_state = SharedMap::new(packet_filters, file_open_allow, config_src.clone());
    let server = Server::new(listener, shared_state, config_src);

    log::info!("Enter Ctrl-C to shutdown");

    tokio::select! {
        result = server.start() => {
            result?;
        }
        _ = signal::ctrl_c() => {}
    }

    Ok(())
}
