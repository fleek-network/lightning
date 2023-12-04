use std::fs;
use std::sync::Arc;
use std::time::{Duration, SystemTime};

use anyhow::Result;
use fleek_crypto::{NodeSecretKey, SecretKey};
use lightning_application::app::Application;
use lightning_dht::Dht;
use lightning_e2e::swarm::Swarm;
use lightning_e2e::utils::logging;
use lightning_e2e::utils::networking::PortAssigner;
use lightning_interfaces::infu_collection::Collection;
use lightning_interfaces::types::{Blake3Hash, KeyPrefix};
use lightning_interfaces::{partial, DhtInterface};
use lightning_node::FinalTypes;
use lightning_topology::Topology;
use resolved_pathbuf::ResolvedPathBuf;
use serial_test::serial;
use tokio::sync::Notify;

partial!(PartialBinding {
    ApplicationInterface = Application<Self>;
    TopologyInterface = Topology<Self>;
    DhtInterface = Dht<Self>;
});

#[tokio::test]
#[serial]
async fn e2e_dht_put_and_get() -> Result<()> {
    logging::setup();

    // Start epoch now and let it end in 20 seconds.
    let epoch_start = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64;

    let port_assigner = PortAssigner::default();

    // Start bootstrapper
    let bootstrap_secret_key = NodeSecretKey::generate();
    let bootstrap_shutdown_notify = Arc::new(Notify::new());
    let bootstrap_ready = Arc::new(Notify::new());

    // Wait for bootstrapper to start
    bootstrap_ready.notified().await;

    let path = ResolvedPathBuf::try_from("~/.lightning-test/e2e/dht").unwrap();
    if path.exists() {
        fs::remove_dir_all(&path).expect("Failed to clean up swarm directory before test.");
    }
    let swarm = Swarm::builder()
        .with_directory(path)
        .with_min_port(10301)
        .with_max_port(10400)
        .with_num_nodes(4)
        .with_epoch_start(epoch_start)
        .with_bootstrappers(vec![bootstrap_secret_key.to_pk()])
        .with_port_assigner(port_assigner)
        .build();
    swarm.launch().await.unwrap();

    // Wait a bit for the nodes to start.
    tokio::time::sleep(Duration::from_secs(5)).await;

    let key: Blake3Hash = rand::random();
    let value: [u8; 4] = rand::random();

    #[allow(clippy::filter_map_identity)]
    let dht_sockets: Vec<Dht<FinalTypes>> = swarm
        .get_dht_sockets()
        .into_iter()
        .filter_map(|s| s)
        .collect();

    // Send DHT put to an arbitrary node in the swarm
    dht_sockets[0].put(KeyPrefix::ContentRegistry, key.as_ref(), value.as_ref());

    // Wait some time for the DHT to do its magic
    tokio::time::sleep(Duration::from_secs(5)).await;

    // Perform a DHT lookup on every node in the swarm
    for dht_socket in dht_sockets {
        let res = dht_socket
            .get(KeyPrefix::ContentRegistry, key.as_ref())
            .await;
        match res {
            Some(entry) => {
                // Make sure the retrieved value equals the value we stored
                assert_eq!(value.to_vec(), entry.value);
            },
            _ => panic!("Unexpected response"),
        }
    }

    bootstrap_shutdown_notify.notify_one();
    swarm.shutdown();
    Ok(())
}
