use std::collections::{BTreeSet, HashMap};
use std::fs;
use std::time::{Duration, SystemTime};

use anyhow::Result;
use fleek_crypto::NodePublicKey;
use hp_fixed::unsigned::HpUfixed;
use lightning_e2e::swarm::{Swarm, SwarmNode};
use lightning_e2e::utils::rpc;
use lightning_interfaces::types::Staking;
use lightning_test_utils::logging;
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
        .with_min_port(10100)
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

    swarm.shutdown().await;
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
        .with_min_port(10200)
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

    swarm.shutdown().await;
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
        .with_min_port(10300)
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

    swarm.shutdown().await;
    Ok(())
}

#[tokio::test]
#[serial]
async fn e2e_test_staking_auction() -> Result<()> {
    logging::setup();

    // Start epoch now and let it end in 40 seconds.
    let epoch_start = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64;

    let path = ResolvedPathBuf::try_from("~/.lightning-test/e2e/staking-auction").unwrap();
    if path.exists() {
        fs::remove_dir_all(&path).expect("Failed to clean up swarm directory before test.");
    }

    // Set a node with high rep and a slightly lower stake then everyone else
    let high_rep_node = SwarmNode {
        reputation_score: Some(99),
        stake: Some(Staking {
            staked: HpUfixed::<18>::from(1000_u64),
            ..Default::default()
        }),
        is_committee: true,
    };

    // Set a node with low rep and the same stake as the previous node. This node should lose the
    // auction
    let low_rep_node = SwarmNode {
        reputation_score: Some(20),
        stake: Some(Staking {
            staked: HpUfixed::<18>::from(1000_u64),
            ..Default::default()
        }),
        is_committee: true,
    };

    // Spawn swarm with initially 1 more node than the node_count_param. This should cause one node
    // to be kicked off on epoch change
    let swarm = Swarm::builder()
        .with_directory(path)
        .with_min_port(10400)
        .with_num_nodes(5)
        .with_node_count_param(4)
        .with_epoch_time(20000)
        .with_epoch_start(epoch_start)
        .with_specific_nodes(vec![high_rep_node, low_rep_node])
        .build();

    swarm.launch().await.unwrap();

    // Allow time for nodes to start
    tokio::time::sleep(Duration::from_secs(5)).await;

    // Get the stakes to figure out who our low staked nodes are
    let stake_map: HashMap<NodePublicKey, Staking> = swarm.get_genesis_stakes();
    let low_stake_nodes: Vec<&NodePublicKey> = stake_map
        .iter()
        .filter_map(|node| {
            if node.1.staked == 1000_u64.into() {
                Some(node.0)
            } else {
                None
            }
        })
        .collect();

    // Make sure only 2 nodes have that stake
    assert!(low_stake_nodes.len() == 2);

    // Find rpc endpoint that is not one of our test nodes
    let rpc_addresses = swarm.get_rpc_addresses();
    let rpc_endpoint = rpc_addresses
        .iter()
        .find(|node| node.0 != low_stake_nodes[0] && node.0 != low_stake_nodes[1])
        .unwrap();

    // Wait for epoch to change.
    tokio::time::sleep(Duration::from_secs(30)).await;

    // Get the new committee after the epoch change
    let committee_member_request = json!({
        "jsonrpc": "2.0",
        "method":"flk_get_committee_members",
        "params":[],
        "id":1,
    });

    let response = rpc::rpc_request(rpc_endpoint.1.clone(), committee_member_request.to_string())
        .await
        .unwrap();

    let current_committee: BTreeSet<NodePublicKey> =
        rpc::parse_response::<Vec<NodePublicKey>>(response)
            .await
            .expect("Failed to parse response.")
            .into_iter()
            .collect();

    current_committee
        .iter()
        .for_each(|node| println!("{:?}", node));

    // Figure out the rep of our low staked nodes so we know which one shouldnt be on the committee
    let rep_request_one = json!({
        "jsonrpc": "2.0",
        "method":"flk_get_reputation",
        "params":[low_stake_nodes[0].clone()],
        "id":1,
    });
    let rep_request_two = json!({
        "jsonrpc": "2.0",
        "method":"flk_get_reputation",
        "params":[low_stake_nodes[1].clone()],
        "id":1,
    });

    let response_one = rpc::rpc_request(rpc_endpoint.1.clone(), rep_request_one.to_string())
        .await
        .unwrap();
    let response_two = rpc::rpc_request(rpc_endpoint.1.clone(), rep_request_two.to_string())
        .await
        .unwrap();

    let rep_one: Option<u8> = rpc::parse_response::<Option<u8>>(response_one)
        .await
        .expect("Failed to parse response.");
    let rep_two: Option<u8> = rpc::parse_response::<Option<u8>>(response_two)
        .await
        .expect("Failed to parse response.");

    // Make sure the lower reputation node lost the tiebreaker and is not on the active node list
    if rep_one.unwrap() <= rep_two.unwrap() {
        assert!(!current_committee.contains(low_stake_nodes[0]));
    } else {
        assert!(!current_committee.contains(low_stake_nodes[1]));
    }

    swarm.shutdown().await;
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
