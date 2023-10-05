use std::collections::{BTreeSet, HashMap};
use std::fs;
use std::time::{Duration, SystemTime};

use anyhow::Result;
use fleek_crypto::NodePublicKey;
use lightning_e2e::swarm::Swarm;
use lightning_e2e::utils::{logging, rpc};
use resolved_pathbuf::ResolvedPathBuf;
use serde_json::json;
use serial_test::serial;

#[tokio::test]
#[serial]
async fn e2e_epoch_change_all_nodes_on_committee() -> Result<()> {
    logging::setup();

    // Start epoch now and let it end in 40 seconds.
    let epoch_start = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64;

    let path = ResolvedPathBuf::try_from("~/.lightning-test/e2e/epoch-change-committee").unwrap();
    if path.exists() {
        fs::remove_dir_all(&path).expect("Failed to clean up swarm directory before test.");
    }
    let swarm = Swarm::builder()
        .with_directory(path)
        .with_min_port(10000)
        .with_max_port(10100)
        .with_num_nodes(4)
        .with_epoch_time(30000)
        .with_epoch_start(epoch_start)
        .build();
    swarm.launch().await.unwrap();

    // Wait a bit for the nodes to start.
    tokio::time::sleep(Duration::from_secs(5)).await;

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
        assert_eq!(epoch, 0);
    }

    // The epoch will change after 40 seconds, and we already waited 5 seconds.
    // To give some time for the epoch change, we will wait another 30 seconds here.
    tokio::time::sleep(Duration::from_secs(30)).await;

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

    swarm.shutdown();
    Ok(())
}

#[tokio::test]
#[serial]
async fn e2e_epoch_change_with_edge_node() -> Result<()> {
    logging::setup();

    // Start epoch now and let it end in 40 seconds.
    let epoch_start = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64;

    let path = ResolvedPathBuf::try_from("~/.lightning-test/e2e/epoch-change-edge-node").unwrap();
    if path.exists() {
        fs::remove_dir_all(&path).expect("Failed to clean up swarm directory before test.");
    }
    let swarm = Swarm::builder()
        .with_directory(path)
        .with_min_port(10101)
        .with_max_port(10200)
        .with_num_nodes(5)
        .with_committee_size(4)
        .with_epoch_time(30000)
        .with_epoch_start(epoch_start)
        .build();
    swarm.launch().await.unwrap();

    // Wait a bit for the nodes to start.
    tokio::time::sleep(Duration::from_secs(5)).await;

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
        assert_eq!(epoch, 0);
    }

    // The epoch will change after 40 seconds, and we already waited 5 seconds.
    // To give some time for the epoch change, we will wait another 30 seconds here.
    tokio::time::sleep(Duration::from_secs(30)).await;

    let request = json!({
        "jsonrpc": "2.0",
        "method":"flk_get_epoch",
        "params":[],
        "id":1,
    });
    for (_key, address) in swarm.get_rpc_addresses() {
        let response = rpc::rpc_request(address, request.to_string())
            .await
            .unwrap();

        let epoch = rpc::parse_response::<u64>(response)
            .await
            .expect("Failed to parse response.");
        assert_eq!(epoch, 1);
    }
    Ok(())
}

#[tokio::test]
#[serial]
async fn e2e_committee_change() -> Result<()> {
    logging::setup();

    // Start epoch now and let it end in 40 seconds.
    let epoch_start = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64;

    let path = ResolvedPathBuf::try_from("~/.lightning-test/e2e/committee-change").unwrap();
    if path.exists() {
        fs::remove_dir_all(&path).expect("Failed to clean up swarm directory before test.");
    }

    let committee_size = 4;
    let swarm = Swarm::builder()
        .with_directory(path)
        .with_min_port(10201)
        .with_max_port(10300)
        .with_num_nodes(5)
        .with_committee_size(committee_size)
        .with_epoch_time(20000)
        .with_epoch_start(epoch_start)
        .build();
    swarm.launch().await.unwrap();

    // Wait a bit for the nodes to start.
    tokio::time::sleep(Duration::from_secs(5)).await;

    // Make sure all nodes start with the same committee.
    compare_committee(swarm.get_rpc_addresses(), committee_size as usize).await;

    // Wait for epoch to change.
    tokio::time::sleep(Duration::from_secs(30)).await;

    // Make sure all nodes still all have the same committee.
    compare_committee(swarm.get_rpc_addresses(), committee_size as usize).await;
    Ok(())
}

async fn compare_committee(
    rpc_addresses: HashMap<NodePublicKey, String>,
    committee_size: usize,
) -> BTreeSet<NodePublicKey> {
    let request = json!({
        "jsonrpc": "2.0",
        "method":"flk_get_committee_members",
        "params":[],
        "id":1,
    });

    let rpc_addresses: Vec<(NodePublicKey, String)> = rpc_addresses.into_iter().collect();

    let response = rpc::rpc_request(rpc_addresses[0].1.clone(), request.to_string())
        .await
        .unwrap();
    let target_committee: BTreeSet<NodePublicKey> =
        rpc::parse_response::<Vec<NodePublicKey>>(response)
            .await
            .expect("Failed to parse response.")
            .into_iter()
            .collect();

    // Make sure that the committee size equals the configured size.
    assert_eq!(target_committee.len(), committee_size);

    for (_, address) in rpc_addresses.iter() {
        if &rpc_addresses[0].1 == address {
            continue;
        }
        let response = rpc::rpc_request(address.clone(), request.to_string())
            .await
            .unwrap();
        let committee: BTreeSet<NodePublicKey> =
            rpc::parse_response::<Vec<NodePublicKey>>(response)
                .await
                .expect("Failed to parse response.")
                .into_iter()
                .collect();

        // Make sure all nodes have the same committee
        assert_eq!(target_committee, committee);
    }
    target_committee
}
