use std::fs;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::SystemTime;

use anyhow::Result;
use clap::Parser;
use fleek_crypto::{NodeSecretKey, PublicKey, SecretKey};
use lightning_application::app::Application;
use lightning_dht::Dht;
use lightning_e2e::swarm::Swarm;
use lightning_e2e::utils::networking::{PortAssigner, Transport};
use lightning_e2e::utils::{logging, shutdown};
use lightning_interfaces::infu_collection::Collection;
use lightning_interfaces::partial;
use lightning_topology::Topology;
use resolved_pathbuf::ResolvedPathBuf;
use tokio::sync::Notify;

partial!(PartialBinding {
    ApplicationInterface = Application<Self>;
    TopologyInterface = Topology<Self>;
    DhtInterface = Dht<Self>;
});

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Cli {
    /// Number of nodes to spawn
    #[arg(short, long, default_value_t = 4)]
    num_nodes: usize,

    /// Number of committee members.
    #[arg(short, long, default_value_t = 4)]
    committee_size: usize,

    /// Epoch duration in millis
    #[arg(short, long, default_value_t = 60000)]
    epoch_time: u64,
}

#[tokio::main]
async fn main() -> Result<()> {
    logging::setup();

    let args = Cli::parse();

    if args.committee_size > args.num_nodes {
        panic!("Committee size can not be larger than number of nodes.")
    }

    // Start bootstrapper
    let mut port_assigner = PortAssigner::default();
    let bootstrapper_port = port_assigner
        .get_port(12001, 13000, Transport::Udp)
        .expect("Failed to assign port");

    // Todo: get IP from application.
    let bootstrapper_address: SocketAddr =
        format!("127.0.0.1:{bootstrapper_port}").parse().unwrap();
    let bootstrap_secret_key = NodeSecretKey::generate();
    let bootstrap_ready = Arc::new(Notify::new());

    // Wait for bootstrapper to start
    bootstrap_ready.notified().await;

    let epoch_start = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64;

    let path = ResolvedPathBuf::try_from("~/.lightning-test/e2e/spawn-swarm").unwrap();
    if path.exists() {
        fs::remove_dir_all(&path).expect("Failed to clean up swarm directory before test.");
    }
    let swarm = Swarm::builder()
        .with_directory(path)
        .with_min_port(12001)
        .with_max_port(13000)
        .with_num_nodes(args.num_nodes)
        .with_committee_size(args.committee_size as u64)
        .with_epoch_time(args.epoch_time)
        .with_epoch_start(epoch_start)
        .with_archiver()
        .with_bootstrappers(vec![bootstrap_secret_key.to_pk()])
        .with_port_assigner(port_assigner)
        .build();
    swarm.launch().await.unwrap();

    let mut s = String::from("#####################################\n\n");
    s.push_str(&format!(
        "DHT Bootstrapper Address: {bootstrapper_address}\n"
    ));
    s.push_str(&format!(
        "DHT Bootstrapper Public Key: {}\n\n",
        bootstrap_secret_key.to_pk().to_base58()
    ));

    s.push_str("#####################################\n\n");
    for (pub_key, rpc_address) in swarm.get_rpc_addresses() {
        s.push_str(&format!(
            "Public Key: {}\nRPC Address: {}\n\n",
            pub_key.to_base58(),
            rpc_address
        ));
    }
    s.push_str("#####################################");
    println!("{s}");

    shutdown::shutdown_stream().await;

    Ok(())
}
