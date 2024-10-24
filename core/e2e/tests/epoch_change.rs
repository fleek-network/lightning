use std::collections::{BTreeSet, HashMap};
use std::time::{Duration, SystemTime};

use fleek_crypto::NodePublicKey;
use hp_fixed::unsigned::HpUfixed;
use lightning_e2e::swarm::{Swarm, SwarmNode};
use lightning_interfaces::types::Staking;
use lightning_rpc::interface::Fleek;
use lightning_rpc::RpcClient;
use lightning_test_utils::logging;
use tempfile::tempdir;

#[tokio::test]
async fn e2e_epoch_change_all_nodes_on_committee() {
    logging::setup();

    // Initialize the swarm.
    let temp_dir = tempdir().unwrap();
    let mut swarm = Swarm::builder()
        .with_directory(temp_dir.path().to_path_buf().try_into().unwrap())
        .with_min_port(10100)
        .with_num_nodes(4)
        // We need to include enough time in this epoch time for the nodes to start up, or else it
        // begins the epoch change immediately when they do. We can even get into a situation where
        // another epoch change starts quickly after that, causing our expectation of epoch = 1
        // below to fail.
        .with_epoch_time(15000)
        .with_epoch_start(
            SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap()
                .as_millis() as u64,
        )
        .build();

    // Launch genesis committee and wait for RPC to be ready on them.
    swarm.launch_genesis_committee().await.unwrap();
    swarm.wait_for_rpc_ready_genesis_committee().await;

    // Launch the non-committee node now that the committee/bootstrap nodes are up, and wait for RPC
    // to be ready on them.
    swarm.launch_non_genesis_committee().await.unwrap();
    swarm.wait_for_rpc_ready_non_genesis_committee().await;

    // Wait for RPC to be ready.
    swarm.wait_for_rpc_ready().await;

    // Check epoch across all nodes.
    for (_, client) in swarm.get_rpc_clients() {
        let epoch = client.get_epoch().await.unwrap();
        assert_eq!(epoch, 0);
    }

    // Wait for epoch to change across all nodes.
    swarm
        .wait_for_epoch_change(1, Duration::from_secs(60))
        .await
        .unwrap();

    // Shutdown the swarm.
    swarm.shutdown().await;
}

#[tokio::test]
async fn e2e_epoch_change_with_some_nodes_not_on_committee() {
    logging::setup();

    // Initialize the swarm.
    let temp_dir = tempdir().unwrap();
    let mut swarm = Swarm::builder()
        .with_directory(temp_dir.path().to_path_buf().try_into().unwrap())
        .with_min_port(10200)
        .with_num_nodes(5)
        .with_committee_size(4)
        // We need to include enough time in this epoch time for the nodes to start up, or else it
        // begins the epoch change immediately when they do. We can even get into a situation where
        // another epoch change starts quickly after that, causing our expectation of epoch = 1
        // below to fail.
        .with_epoch_time(15000)
        .with_epoch_start(
            SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap()
                .as_millis() as u64,
        )
        .build();

    // Launch genesis committee and wait for RPC to be ready on them.
    swarm.launch_genesis_committee().await.unwrap();
    swarm.wait_for_rpc_ready_genesis_committee().await;

    // Launch the non-committee node now that the committee/bootstrap nodes are up, and wait for RPC
    // to be ready on them.
    swarm.launch_non_genesis_committee().await.unwrap();
    swarm.wait_for_rpc_ready_non_genesis_committee().await;

    // Check that the initial epoch is 0 across all nodes.
    for (_, client) in swarm.get_rpc_clients() {
        assert_eq!(client.get_epoch().await.unwrap(), 0);
    }

    // Check that the committee is the same across all nodes.
    let committees = swarm.get_committee_by_node().await.unwrap();
    for (i, committee) in &committees {
        for (j, other_committee) in &committees {
            if i != j {
                assert_eq!(committee, other_committee);
            }
        }
    }

    // Wait for epoch to change across all nodes.
    swarm
        .wait_for_epoch_change(1, Duration::from_secs(60))
        .await
        .unwrap();

    // Check that the committee is the same across all nodes.
    let committees = swarm.get_committee_by_node().await.unwrap();
    for (i, committee) in &committees {
        for (j, other_committee) in &committees {
            if i != j {
                assert_eq!(committee, other_committee);
            }
        }
    }

    // Shutdown the swarm.
    swarm.shutdown().await;
}

#[tokio::test]
async fn e2e_test_staking_auction() {
    logging::setup();

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
    let temp_dir = tempdir().unwrap();
    let mut swarm = Swarm::builder()
        .with_directory(temp_dir.path().to_path_buf().try_into().unwrap())
        .with_min_port(10400)
        .with_num_nodes(5)
        .with_node_count_param(4)
        // We need to include enough time in this epoch time for the nodes to start up and for
        // enough pings to be successful on each, or else that node may be the one that is not on
        // the next committee and cause our expectation below to fail.
        .with_epoch_time(15000)
        .with_epoch_start(
            SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap()
                .as_millis() as u64,
        )
        .with_specific_nodes(vec![high_rep_node, low_rep_node])
        .build();

    // Launch the genesis committee and wait for RPC to be ready on them.
    swarm.launch_genesis_committee().await.unwrap();
    swarm.wait_for_rpc_ready_genesis_committee().await;

    // Launch the non-committee node now that the committee/bootstrap nodes are up, and wait for RPC
    // to be ready on them.
    swarm.launch_non_genesis_committee().await.unwrap();
    swarm.wait_for_rpc_ready_non_genesis_committee().await;

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

    // Wait for epoch to change across all nodes.
    swarm
        .wait_for_epoch_change(1, Duration::from_secs(60))
        .await
        .unwrap();

    let client = RpcClient::new_no_auth(rpc_endpoint.1).unwrap();
    let response = client.get_committee_members(None).await.unwrap();
    let current_committee: BTreeSet<NodePublicKey> = response.into_iter().collect();

    let rep_one = client
        .get_reputation(*low_stake_nodes[0], None)
        .await
        .unwrap();
    let rep_two = client
        .get_reputation(*low_stake_nodes[1], None)
        .await
        .unwrap();

    // Make sure the lower reputation node lost the tiebreaker and is not on the active node list
    if rep_one <= rep_two {
        assert!(!current_committee.contains(low_stake_nodes[0]));
    } else {
        assert!(!current_committee.contains(low_stake_nodes[1]));
    }

    swarm.shutdown().await;
}
