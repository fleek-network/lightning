use std::fs;
use std::time::SystemTime;

use anyhow::Result;
use clap::Parser;
use lightning_application::app::Application;
use lightning_e2e::swarm::Swarm;
use lightning_e2e::utils::shutdown;
use lightning_interfaces::prelude::*;
use lightning_interfaces::types::ServiceId;
use lightning_service_executor::shim::ServiceExecutor;
use lightning_test_utils::config::LIGHTNING_TEST_HOME_DIR;
use lightning_test_utils::logging;
use lightning_topology::Topology;
use resolved_pathbuf::ResolvedPathBuf;

partial_node_components!(PartialBinding {
    ApplicationInterface = Application<Self>;
    TopologyInterface = Topology<Self>;
    ServiceExecutorInterface = ServiceExecutor<Self>;
});

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Cli {
    /// Number of nodes to spawn
    #[arg(short, long, default_value_t = 4)]
    num_nodes: usize,

    /// Number of committee members
    #[arg(short, long, default_value_t = 4)]
    committee_size: usize,

    /// Epoch duration in millis
    #[arg(short, long, default_value_t = 60000)]
    epoch_time: u64,

    /// Use persistence for the application state
    #[arg(short, long, default_value_t = false)]
    persistence: bool,

    /// The services that are running on each node
    #[arg(short, long)]
    services: Vec<ServiceId>,
}

fn main() -> Result<()> {
    logging::setup();

    let args = Cli::parse();

    if args.committee_size > args.num_nodes {
        panic!("Committee size can not be larger than number of nodes.")
    }

    if let Ok(service_id) = std::env::var("SERVICE_ID") {
        // In case of spawning the binary with the `SERVICE_ID` env abort the default flow and
        // instead run the code for that service. We avoid using a runtime so that a service can use
        // its own.
        <c!(PartialBinding::ServiceExecutorInterface)>::run_service(
            service_id.parse().expect("SERVICE_ID to be a number"),
        );
        std::process::exit(0);
    }

    let fut = async {
        let epoch_start = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;

        let path =
            ResolvedPathBuf::try_from(LIGHTNING_TEST_HOME_DIR.join("e2e/spawn-swarm")).unwrap();
        if path.exists() {
            fs::remove_dir_all(&path).expect("Failed to clean up swarm directory before test.");
        }
        let mut swarm = Swarm::builder()
            .with_directory(path)
            .with_min_port(12000)
            .with_num_nodes(args.num_nodes)
            .with_committee_size(args.committee_size as u64)
            .with_epoch_time(args.epoch_time)
            .with_epoch_start(epoch_start)
            .with_archiver()
            .persistence(args.persistence)
            .with_services(args.services)
            .build();
        swarm.launch().await.unwrap();

        let mut s = String::from("#####################################\n\n");
        for (pub_key, ports) in swarm.get_ports() {
            s.push_str(&format!("Public Key: {pub_key}\n{ports}\n\n"));
        }
        s.push_str("#####################################");
        println!("{s}");

        shutdown::shutdown_stream().await;
        Ok(())
    };

    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .expect("failed to initialize runtime")
        .block_on(fut)
}
