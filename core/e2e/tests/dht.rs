use std::sync::Arc;
use std::thread;
use std::time::{Duration, SystemTime};

use anyhow::Result;
use fleek_crypto::{NodeSecretKey, SecretKey};
use lightning_application::app::Application;
use lightning_dht::config::{Bootstrapper, Config as DhtConfig};
use lightning_dht::dht::{Builder as DhtBuilder, Dht};
use lightning_e2e::swarm::Swarm;
use lightning_e2e::utils::networking::{PortAssigner, Transport};
use lightning_e2e::utils::rpc;
use lightning_interfaces::infu_collection::Collection;
use lightning_interfaces::types::TableEntry;
use lightning_interfaces::{partial, Blake3Hash, WithStartAndShutdown};
use lightning_topology::Topology;
use resolved_pathbuf::ResolvedPathBuf;
use serde_json::json;
use serial_test::serial;
use tokio::sync::Notify;

partial!(PartialBinding {
    ApplicationInterface = Application<Self>;
    TopologyInterface = Topology<Self>;
    DhtInterface = Dht<Self>;
});

#[tokio::test]
#[serial]
async fn e2e_dht() -> Result<()> {
    // Start epoch now and let it end in 20 seconds.
    let epoch_start = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64;

    let mut port_assigner = PortAssigner::default();
    let bootstrapper_port = port_assigner
        .get_port(11001, 12000, Transport::Udp)
        .expect("Failed to assign port");

    // Todo: get IP from application.
    let bootstrapper_address = format!("127.0.0.1:{bootstrapper_port}").parse().unwrap();

    let bootstrapper_config = DhtConfig {
        address: bootstrapper_address,
        bootstrappers: vec![],
    };

    // Start bootstrapper
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

    let path = ResolvedPathBuf::try_from("~/.lightning-test/e2e/dht").unwrap();
    let swarm = Swarm::builder()
        .with_directory(path)
        .with_min_port(10201)
        .with_max_port(10300)
        .with_num_nodes(4)
        .with_epoch_start(epoch_start)
        .with_bootstrappers(vec![Bootstrapper {
            address: bootstrapper_address,
            network_public_key: bootstrap_secret_key.to_pk(),
        }])
        .with_port_assigner(port_assigner)
        .build();
    swarm.launch().await.unwrap();

    // Wait a bit for the nodes to start.
    tokio::time::sleep(Duration::from_secs(5)).await;

    let key: Blake3Hash = rand::random();
    let value: [u8; 4] = rand::random();

    let request = json!({
        "jsonrpc": "2.0",
        "method":"flk_dht_put",
        "params": {"key": key.to_vec(), "value": value.to_vec()},
        "id":1,
    });

    // Send DHT put to an arbitrary node in the swarm
    let rpc_addresses: Vec<String> = swarm.get_rpc_addresses().into_values().collect();
    let response = rpc::rpc_request(rpc_addresses[0].clone(), request.to_string())
        .await
        .unwrap();
    rpc::parse_response::<()>(response)
        .await
        .expect("Failed to parse response.");

    // Wait some time for the DHT to do its magic
    tokio::time::sleep(Duration::from_secs(5)).await;

    // Perform a DHT lookup on every node in the swarm
    let request = json!({
        "jsonrpc": "2.0",
        "method":"flk_dht_get",
        "params": {"key": key.to_vec()},
        "id":1,
    });
    for (_, address) in swarm.get_rpc_addresses() {
        let response = rpc::rpc_request(address, request.to_string())
            .await
            .unwrap();

        let entry = rpc::parse_response::<Option<TableEntry>>(response)
            .await
            .expect("Failed to parse response.");
        let entry = entry.expect("Value not found in DHT");
        // Make sure the retrieved value equals the value we stored
        assert_eq!(value.to_vec(), entry.value);
    }
    bootstrap_shutdown_notify.notify_one();
    Ok(())
}
