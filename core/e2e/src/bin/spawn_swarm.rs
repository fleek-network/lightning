use std::sync::Arc;
use std::time::SystemTime;
use std::{fs, thread};

use anyhow::Result;
use clap::Parser;
use fleek_crypto::{NodeSecretKey, PublicKey, SecretKey};
use lightning_application::app::Application;
use lightning_dht::config::{Bootstrapper, Config as DhtConfig};
use lightning_dht::dht::{Builder as DhtBuilder, Dht};
use lightning_e2e::swarm::Swarm;
use lightning_e2e::utils::networking::{PortAssigner, Transport};
use lightning_e2e::utils::{logging, shutdown};
use lightning_interfaces::infu_collection::Collection;
use lightning_interfaces::{partial, WithStartAndShutdown};
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
    let bootstrapper_address = format!("127.0.0.1:{bootstrapper_port}").parse().unwrap();
    let bootstrapper_config = DhtConfig {
        address: bootstrapper_address,
        bootstrappers: vec![],
    };
    let bootstrap_secret_key = NodeSecretKey::generate();
    let bootstrap_shutdown_notify = Arc::new(Notify::new());
    let bootstrap_ready = Arc::new(Notify::new());
    let bootstrap_ready_rx = bootstrap_ready.clone();
    let bootstrap_shutdown_notify_rx = bootstrap_shutdown_notify.clone();

    let key_cloned = bootstrap_secret_key.clone();
    let _bootstrap_handle = thread::spawn(move || {
        let mut builder = tokio::runtime::Builder::new_multi_thread();
        let runtime = builder
            .enable_all()
            .build()
            .expect("Failed to build tokio runtime for node container.");

        runtime.block_on(async move {
            let builder = DhtBuilder::<PartialBinding>::new(
                key_cloned,
                bootstrapper_config,
                Default::default(),
                Default::default(),
            );
            let dht = builder.build().unwrap();
            dht.start().await;
            bootstrap_ready_rx.notify_one();

            bootstrap_shutdown_notify_rx.notified().await;
            dht.shutdown().await;
        });
    });

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
        .with_bootstrappers(vec![Bootstrapper {
            address: bootstrapper_address,
            network_public_key: bootstrap_secret_key.to_pk(),
        }])
        .with_port_assigner(port_assigner)
        .build();
    swarm.launch().await.unwrap();

    let mut s = String::from("#####################################\n\n");
    s.push_str(&format!(
        "DHT Bootstrapper Address: {bootstrapper_address}\n"
    ));
    s.push_str(&format!(
        "DHT Bootstrapper Public Key: {}\n\n",
        bootstrap_secret_key.to_pk().to_base64()
    ));

    s.push_str("#####################################\n\n");
    for (pub_key, rpc_address) in swarm.get_rpc_addresses() {
        s.push_str(&format!(
            "Public Key: {}\nRPC Address: {}\n\n",
            pub_key.to_base64(),
            rpc_address
        ));
    }
    s.push_str("#####################################");
    println!("{s}");

    shutdown::shutdown_stream().await;

    Ok(())
}
