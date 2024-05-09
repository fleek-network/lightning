use std::fs;
use std::time::{Duration, SystemTime};

use anyhow::Result;
use lightning_e2e::swarm::Swarm;
use lightning_e2e::utils::rpc;
use lightning_interfaces::types::Epoch;
use lightning_test_utils::logging;
use resolved_pathbuf::ResolvedPathBuf;
use serde_json::json;
use serial_test::serial;

#[tokio::test]
#[serial]
async fn e2e_checkpoint() -> Result<()> {
    logging::setup();

    // Start epoch now and let it end in 40 seconds.
    let epoch_start = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64;

    let path = ResolvedPathBuf::try_from("~/.lightning-test/e2e/checkpoint").unwrap();
    if path.exists() {
        fs::remove_dir_all(&path).expect("Failed to clean up swarm directory before test.");
    }
    let swarm = Swarm::builder()
        .with_directory(path)
        .with_min_port(10000)
        .with_num_nodes(4)
        .with_epoch_time(30000)
        .with_epoch_start(epoch_start)
        .persistence(true)
        .build();
    swarm.launch().await.unwrap();

    // Wait for the epoch to change.
    tokio::time::sleep(Duration::from_secs(35)).await;

    let request = json!({
        "jsonrpc": "2.0",
        "method":"flk_get_epoch",
        "params":[],
        "id":1,
    });
    for (_, address) in swarm.get_rpc_addresses() {
        let response = rpc::rpc_request(address, request.to_string())
            .await
            .unwrap();

        let epoch = rpc::parse_response::<u64>(response)
            .await
            .expect("Failed to parse response.");
        assert_eq!(epoch, 1);
    }

    let request = json!({
        "jsonrpc": "2.0",
        "method":"flk_get_last_epoch_hash",
        "params":[],
        "id":1,
    });
    let mut target_hash = None;
    for (_, address) in swarm.get_rpc_addresses() {
        let response = rpc::rpc_request(address, request.to_string())
            .await
            .unwrap();

        let (epoch_hash, _) = rpc::parse_response::<([u8; 32], Epoch)>(response)
            .await
            .expect("Failed to parse response.");
        if target_hash.is_none() {
            target_hash = Some(epoch_hash);
            // Make sure that we stored an epoch hash.
            assert_ne!(target_hash.unwrap(), [0; 32]);
        }

        // Make sure that all nodes stored the same hash for the epoch state.
        assert_eq!(epoch_hash, target_hash.unwrap());
    }
    // TODO(matthias): read the block stores of all the nodes and make sure they all stored the
    // checkpoint

    swarm.shutdown().await;
    Ok(())
}
