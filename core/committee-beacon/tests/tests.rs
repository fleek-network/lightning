use std::collections::{HashMap, HashSet};
use std::time::Duration;

use anyhow::Result;
use lightning_committee_beacon::CommitteeBeaconConfig;
use lightning_interfaces::prelude::*;
use lightning_interfaces::types::{
    ExecutionError,
    TransactionReceipt,
    TransactionResponse,
    UpdateMethod,
};
use lightning_interfaces::{
    CommitteeBeaconInterface,
    CommitteeBeaconQueryInterface,
    SyncQueryRunnerInterface,
};
use lightning_test_utils::consensus::MockConsensusConfig;
use lightning_test_utils::e2e::{
    DowncastToTestFullNode,
    TestFullNodeComponentsWithMockConsensus,
    TestFullNodeComponentsWithRealConsensus,
    TestFullNodeComponentsWithRealConsensusWithoutCommitteeBeacon,
    TestFullNodeComponentsWithoutCommitteeBeacon,
    TestNetwork,
    TestNodeBuilder,
};
use lightning_utils::application::QueryRunnerExt;
use lightning_utils::poll::{poll_until, PollUntilError};
use types::{
    CommitteeSelectionBeaconPhase,
    NodeIndex,
    NodeRegistryChange,
    NodeRegistryChangeSlashReason,
    Staking,
};

const COMMIT_PHASE_DURATION: u64 = 8000; // ms
const REVEAL_PHASE_DURATION: u64 = 8000; // ms

#[tokio::test]
async fn test_start_shutdown() {
    let node = lightning_test_utils::e2e::TestNodeBuilder::new()
        .build::<TestFullNodeComponentsWithMockConsensus>(None)
        .await
        .unwrap();
    node.shutdown().await;
}

#[tokio::test]
async fn test_epoch_change_single_node() {
    let commit_phase_duration = COMMIT_PHASE_DURATION;
    let reveal_phase_duration = REVEAL_PHASE_DURATION;

    let mut network = build_network(BuildNetworkOptions {
        committee_nodes: 1,
        commit_phase_duration,
        reveal_phase_duration,
        ..Default::default()
    })
    .await
    .unwrap();
    let node = network
        .node(0)
        .downcast::<TestFullNodeComponentsWithMockConsensus>();

    wait_for_forwarder_to_start().await;

    // Send epoch change transaction from all nodes.
    let epoch = network.change_epoch().await.unwrap();

    // Check that we are in the commit phase.
    wait_for_committee_selection_beacon_phase(&network, |phase| {
        matches!(phase, Some(CommitteeSelectionBeaconPhase::Commit((0, 0))))
    })
    .await
    .unwrap();

    // Wait for the commit phase to end, then submit the commit timeout txn.
    tokio::time::sleep(Duration::from_millis(commit_phase_duration)).await;
    // Send commit phase timeout transaction from all nodes.
    network.commit_phase_timeout(0).await.unwrap();

    // Check that we are in the reveal phase.
    wait_for_committee_selection_beacon_phase(&network, |phase| {
        matches!(phase, Some(CommitteeSelectionBeaconPhase::Reveal((0, 0))))
    })
    .await
    .unwrap();

    // Wait for the reveal phase to end, then submit the reveal timeout txn.
    tokio::time::sleep(Duration::from_millis(reveal_phase_duration)).await;
    network.reveal_phase_timeout(0).await.unwrap();

    // Check that the epoch has been incremented.
    network.wait_for_epoch_change(epoch).await.unwrap();
    let new_epoch = node.get_epoch();
    assert_eq!(new_epoch, epoch);

    // Check that the app state beacons are cleared.
    let beacons = node.app_query().get_committee_selection_beacons();
    assert!(
        beacons.is_empty(),
        "expected beacons to be cleared: {:?}",
        beacons
    );

    // Change epoch again and check that the local database beacons are eventually cleared.
    network.wait_for_epoch_change(epoch).await.unwrap();
    let epoch = network.change_epoch().await.unwrap();

    // Check that we are in the commit phase.
    wait_for_committee_selection_beacon_phase(&network, |phase| {
        matches!(phase, Some(CommitteeSelectionBeaconPhase::Commit((1, 0))))
    })
    .await
    .unwrap();

    // Wait for the commit phase to end, then submit the commit timeout txn.
    tokio::time::sleep(Duration::from_millis(commit_phase_duration)).await;
    // Send commit phase timeout transaction from all nodes.
    network.commit_phase_timeout(0).await.unwrap();
    // Check that we are in the reveal phase.
    wait_for_committee_selection_beacon_phase(&network, |phase| {
        matches!(phase, Some(CommitteeSelectionBeaconPhase::Reveal((1, 0))))
    })
    .await
    .unwrap();

    // Wait for the reveal phase to end, then submit the reveal timeout txn.
    tokio::time::sleep(Duration::from_millis(reveal_phase_duration)).await;
    network.reveal_phase_timeout(0).await.unwrap();
    network.wait_for_epoch_change(epoch).await.unwrap();

    poll_until(
        || async {
            node.committee_beacon()
                .query()
                .get_beacons()
                .is_empty()
                .then_some(())
                .ok_or(PollUntilError::ConditionNotSatisfied)
        },
        Duration::from_secs(3),
        Duration::from_millis(100),
    )
    .await
    .unwrap();

    // Shutdown the network.
    network.shutdown().await;
}

#[tokio::test]
async fn test_epoch_change_multiple_nodes() {
    let commit_phase_duration = COMMIT_PHASE_DURATION;
    let reveal_phase_duration = REVEAL_PHASE_DURATION;

    let mut network = build_network(BuildNetworkOptions {
        committee_nodes: 3,
        commit_phase_duration,
        reveal_phase_duration,
        ..Default::default()
    })
    .await
    .unwrap();

    wait_for_forwarder_to_start().await;

    // Send epoch change transaction from all nodes.
    let epoch = network.change_epoch().await.unwrap();

    // Check that we are in the commit phase.
    wait_for_committee_selection_beacon_phase(&network, |phase| {
        matches!(phase, Some(CommitteeSelectionBeaconPhase::Commit((0, 0))))
    })
    .await
    .unwrap();

    // Wait for the commit phase to end, then submit the commit timeout txn.
    tokio::time::sleep(Duration::from_millis(commit_phase_duration)).await;
    // Send commit phase timeout transaction from all nodes.
    network.commit_phase_timeout(0).await.unwrap();

    // Check that we are in the reveal phase.
    wait_for_committee_selection_beacon_phase(&network, |phase| {
        matches!(phase, Some(CommitteeSelectionBeaconPhase::Reveal((0, 0))))
    })
    .await
    .unwrap();

    // Wait for the reveal phase to end, then submit the reveal timeout txn.
    tokio::time::sleep(Duration::from_millis(reveal_phase_duration)).await;
    network.reveal_phase_timeout(0).await.unwrap();

    // Check that the epoch has been incremented.
    network.wait_for_epoch_change(epoch).await.unwrap();

    // Check that the epoch has been incremented.
    for node in network.nodes() {
        assert_eq!(node.app_query().get_current_epoch(), epoch);
    }

    // Check that the app state beacons are cleared.
    for node in network.nodes() {
        assert!(node
            .app_query()
            .get_committee_selection_beacons()
            .is_empty());
    }

    // Shutdown the network.
    network.shutdown().await;
}

#[tokio::test]
async fn test_insufficient_participation_in_commit_phase() {
    let commit_phase_duration = COMMIT_PHASE_DURATION;
    let reveal_phase_duration = REVEAL_PHASE_DURATION;
    let mut network = build_network(BuildNetworkOptions {
        committee_nodes: 1,
        committee_nodes_without_beacon: 2,
        consensus_buffer_interval: Duration::from_millis(100),
        commit_phase_duration,
        reveal_phase_duration,
        ..Default::default()
    })
    .await
    .unwrap();

    wait_for_forwarder_to_start().await;

    // Execute epoch change transactions from all nodes.
    let epoch = network.node(0).app_query().get_current_epoch();
    network.change_epoch().await.unwrap();

    // Wait for the phase metadata to be set.
    // This should not be necessary but it seems that the data is not always immediately available
    // from the app state query runner after the block is executed.
    wait_for_committee_selection_beacon_phase(&network, |phase| {
        matches!(phase, Some(CommitteeSelectionBeaconPhase::Commit((0, 0))))
    })
    .await
    .unwrap();

    // Wait for the commit phase to end, then submit the commit timeout txn.
    tokio::time::sleep(Duration::from_millis(commit_phase_duration)).await;
    // Send commit phase timeout transaction from all nodes.
    network.commit_phase_timeout(0).await.unwrap();

    // Since only one out of 3 nodes submitted a commit, the commit phase should be restarted, with
    // an incremented round.
    wait_for_committee_selection_beacon_phase(&network, |phase| {
        matches!(phase, Some(CommitteeSelectionBeaconPhase::Commit((0, 1))))
    })
    .await
    .unwrap();

    // Check that the epoch has not changed.
    for node in network.nodes() {
        assert_eq!(node.app_query().get_current_epoch(), epoch);
    }

    // Shutdown the nodes.
    network.shutdown().await;
}

#[tokio::test]
async fn test_single_revealing_node_fully_slashed() {
    let commit_phase_duration = COMMIT_PHASE_DURATION;
    let reveal_phase_duration = REVEAL_PHASE_DURATION;
    let mut network = build_network(BuildNetworkOptions {
        real_consensus: true,
        committee_nodes: 4,
        committee_nodes_without_beacon: 1,
        // The node's initial stake is 1000, and it will be slashed 1000, leaving insufficient stake
        // for a node.
        commit_phase_duration,
        reveal_phase_duration,
        ping_interval: Some(Duration::from_secs(1)),
        ..Default::default()
    })
    .await
    .unwrap();

    // Check that all the nodes are in the initial committee and active node set.
    for node in network.nodes() {
        assert_eq!(
            node.app_query().get_committee_members_by_index(),
            vec![0, 1, 2, 3, 4]
        );
        assert_eq!(
            node.app_query().get_active_node_set(),
            HashSet::from([0, 1, 2, 3, 4])
        );
    }

    wait_for_forwarder_to_start().await;

    // Execute epoch change transactions from all nodes.
    let epoch = network.node(0).app_query().get_current_epoch();
    network.change_epoch().await.unwrap();

    // Wait for the phase metadata to be set.
    // This should not be necessary but it seems that the data is not always immediately available
    // from the app state query runner after the block is executed.
    wait_for_committee_selection_beacon_phase(&network, |phase| {
        matches!(phase, Some(CommitteeSelectionBeaconPhase::Commit((0, 0))))
    })
    .await
    .unwrap();

    // Submit commit transaction from the node that will be non-revealing.
    // We do this manually because the node does not have a committee beacon component running,
    // where all other nodes do.
    let non_revealing_node = network.node(4);
    non_revealing_node
        .execute_transaction_from_node(UpdateMethod::CommitteeSelectionBeaconCommit {
            commit: [0; 32].into(),
        })
        .await
        .unwrap();

    // Wait for the commit phase to end, then submit the commit timeout txn.
    tokio::time::sleep(Duration::from_millis(commit_phase_duration)).await;
    // Since we use the real consensus, the commit phase timeout transaction will be send from the
    // consensus.

    // Wait to transition to the reveal phase.
    wait_for_committee_selection_beacon_phase(&network, |phase| {
        matches!(phase, Some(CommitteeSelectionBeaconPhase::Reveal((0, 0))))
    })
    .await
    .unwrap();

    // Wait for the reveal phase to end, then submit the reveal timeout txn.
    tokio::time::sleep(Duration::from_millis(reveal_phase_duration)).await;
    // Since we use the real consensus, the reveal phase timeout transaction will be send from the
    // consensus.

    // Check that we transition to a new commit phase after reveal phase timeout.
    wait_for_committee_selection_beacon_phase(&network, |phase| {
        matches!(phase, Some(CommitteeSelectionBeaconPhase::Commit((0, 1))))
    })
    .await
    .unwrap();

    for node in network.nodes() {
        // Check that we are in a new round.
        assert_eq!(
            node.app_query().get_committee_selection_beacon_phase(),
            Some(CommitteeSelectionBeaconPhase::Commit((0, 1)))
        );

        // Check that the epoch has not changed.
        assert_eq!(node.app_query().get_current_epoch(), epoch);

        // Check that the non-revealing node has been slashed, and the other has not.
        for i in 0..network.nodes().count() {
            let node_index = i as NodeIndex;
            if node_index == 4 {
                assert_eq!(
                    node.app_query()
                        .get_node_info(&node_index, |n| n.stake.staked)
                        .unwrap(),
                    0u64.into()
                );
            } else {
                assert_eq!(
                    node.app_query()
                        .get_node_info(&node_index, |n| n.stake.staked)
                        .unwrap(),
                    1000u64.into()
                );
            }
        }

        // Check that the node registry changes have been recorded.
        assert_eq!(
            node.app_query().get_committee_members_by_index(),
            vec![0, 1, 2, 3]
        );
        let node_registry_changes = node
            .app_query()
            .get_committee_info(&epoch, |c| c.node_registry_changes)
            .unwrap();
        assert_eq!(node_registry_changes.len(), 2);
        assert_eq!(
            *node_registry_changes.iter().last().unwrap().1,
            vec![(
                network.node(4).get_node_public_key(),
                NodeRegistryChange::Slashed((
                    1000u64.into(),
                    Staking {
                        staked: 0u64.into(),
                        stake_locked_until: 0,
                        locked: 0u64.into(),
                        locked_until: 0,
                    },
                    NodeRegistryChangeSlashReason::CommitteeBeaconNonReveal,
                )),
            )]
        );
    }

    // Wait for narwhal restart after slashing the non-revealing node.
    wait_for_narwhal_restart().await;

    // Submit commit transaction from the non-revealing node and check that it's reverted.
    let receipt = non_revealing_node
        .execute_transaction_from_node_with_receipt(
            UpdateMethod::CommitteeSelectionBeaconCommit {
                commit: [0; 32].into(),
            },
            Duration::from_secs(10),
        )
        .await
        //.unwrap_err();
        .unwrap();

    assert!(matches!(
        receipt,
        TransactionReceipt {
            response: TransactionResponse::Revert(ExecutionError::InsufficientStake),
            ..
        },
    ));

    // Get reputation measurements so that we can check that the non-revealing node is no longer
    // being monitored later.
    let prev_reputation_measurements_by_node = network
        .nodes()
        .map(|n| (n.index(), n.reputation_query().get_measurements()))
        .collect::<HashMap<_, _>>();

    // Sleep for a few seconds so that the pinger has time to send pings.
    tokio::time::sleep(Duration::from_secs(4)).await;

    // Check that the non-revealing node is no longer being monitored while all others are still
    // being monitored.
    for from_node in network.nodes() {
        let from_node_index = from_node.index();

        let prev_reputation_measurements = &prev_reputation_measurements_by_node[&from_node_index];

        // Get the new reputation measurements.
        let new_reputation_measurements = from_node.reputation_query().get_measurements();

        for to_node in network.nodes() {
            let to_node_index = to_node.index();

            if from_node_index == to_node_index {
                continue;
            }

            let prev_measurements = prev_reputation_measurements[&to_node_index]
                .interactions
                .unwrap();
            let new_measurements = new_reputation_measurements[&to_node_index]
                .interactions
                .unwrap();
            if to_node_index == 4 {
                // The non-revealing node should have no new interactions.
                // We allow 2 new interactions in case it raced the removal of the node from the
                // pinger components.
                assert!(
                    new_measurements - prev_measurements <= 2
                        && new_measurements - prev_measurements >= 0,
                    "expected no new interactions for non-revealing node {} from node {} (prev: {:?}, new: {:?})",
                    to_node_index,
                    from_node_index,
                    prev_measurements,
                    new_measurements,
                );
            } else {
                // The other nodes should have new interactions.
                assert!(
                    new_measurements > prev_measurements,
                    "expected new interactions for node {} from node {} (prev: {:?}, new: {:?})",
                    to_node_index,
                    from_node_index,
                    prev_measurements,
                    new_measurements,
                );
            }
        }
    }

    // Shutdown the nodes.
    network.shutdown().await;
}

#[tokio::test]
async fn test_non_revealing_node_partially_slashed_insufficient_stake() {
    let commit_phase_duration = COMMIT_PHASE_DURATION;
    let reveal_phase_duration = REVEAL_PHASE_DURATION;
    let mut network = build_network(BuildNetworkOptions {
        committee_nodes: 4,
        committee_nodes_without_beacon: 1,
        // The node's initial stake is 1000, and it will be slashed 500, leaving insufficient stake
        // for a node.
        min_stake: 1000,
        non_reveal_slash_amount: 500,
        commit_phase_duration,
        reveal_phase_duration,
        real_consensus: true,
        ping_interval: Some(Duration::from_secs(1)),
        ..Default::default()
    })
    .await
    .unwrap();

    wait_for_forwarder_to_start().await;

    // Execute epoch change transactions from all nodes.
    let epoch = network.node(0).app_query().get_current_epoch();
    network.change_epoch().await.unwrap();

    // Wait for the phase metadata to be set.
    // This should not be necessary but it seems that the data is not always immediately available
    // from the app state query runner after the block is executed.
    wait_for_committee_selection_beacon_phase(&network, |phase| {
        matches!(phase, Some(CommitteeSelectionBeaconPhase::Commit((0, 0))))
    })
    .await
    .unwrap();

    // Submit commit transaction from the node that will be non-revealing.
    network
        .node(4)
        .execute_transaction_from_node(UpdateMethod::CommitteeSelectionBeaconCommit {
            commit: [0; 32].into(),
        })
        .await
        .unwrap();

    // Wait for the commit phase to end, then submit the commit timeout txn.
    tokio::time::sleep(Duration::from_millis(commit_phase_duration)).await;
    // Since we use the real consensus, the commit phase timeout transaction will be send from the
    // consensus.

    // Wait to transition to the reveal phase.
    wait_for_committee_selection_beacon_phase(&network, |phase| {
        matches!(phase, Some(CommitteeSelectionBeaconPhase::Reveal((0, 0))))
    })
    .await
    .unwrap();

    // Wait for the reveal phase to end, then submit the reveal timeout txn.
    tokio::time::sleep(Duration::from_millis(reveal_phase_duration)).await;
    // Since we use the real consensus, the reveal phase timeout transaction will be send from the
    // consensus.

    // Check that we transition to a new commit phase after reveal phase timeout.
    wait_for_committee_selection_beacon_phase(&network, |phase| {
        matches!(phase, Some(CommitteeSelectionBeaconPhase::Commit((0, 1))))
    })
    .await
    .unwrap();

    // Check that the epoch has not changed.
    for node in network.nodes() {
        assert_eq!(node.app_query().get_current_epoch(), epoch);
    }

    for node in network.nodes() {
        // Check that the non-revealing node has been slashed, and the other has not.
        assert_eq!(
            node.app_query()
                .get_node_info(&4, |n| n.stake.staked)
                .unwrap(),
            500u64.into()
        );
        assert_eq!(
            node.app_query()
                .get_node_info(&0, |n| n.stake.staked)
                .unwrap(),
            1000u64.into()
        );

        // Check that the node registry changes have been recorded.
        assert_eq!(
            node.app_query().get_committee_members_by_index(),
            vec![0, 1, 2, 3]
        );
        let node_registry_changes = node
            .app_query()
            .get_committee_info(&epoch, |c| c.node_registry_changes)
            .unwrap();
        assert_eq!(node_registry_changes.len(), 2);
        assert_eq!(
            *node_registry_changes.iter().last().unwrap().1,
            vec![(
                network.node(4).get_node_public_key(),
                NodeRegistryChange::Slashed((
                    500u64.into(),
                    Staking {
                        staked: 500u64.into(),
                        stake_locked_until: 0,
                        locked: 0u64.into(),
                        locked_until: 0,
                    },
                    NodeRegistryChangeSlashReason::CommitteeBeaconNonReveal,
                )),
            )]
        );
    }

    // Wait for narwhal restart after slashing the non-revealing node.
    wait_for_narwhal_restart().await;

    // Submit commit transaction from the non-revealing node and check that it's reverted.
    let receipt = network
        .node(4)
        .execute_transaction_from_node_with_receipt(
            UpdateMethod::CommitteeSelectionBeaconCommit {
                commit: [0; 32].into(),
            },
            Duration::from_secs(10),
        )
        .await;

    if let Err(e) = &receipt {
        panic!("ERROR: {e:?}");
    }
    let receipt = receipt.unwrap();

    assert!(matches!(
        receipt,
        TransactionReceipt {
            response: TransactionResponse::Revert(ExecutionError::InsufficientStake),
            ..
        },
    ));

    // Wait for nodes to take reputation measurements.
    tokio::time::sleep(Duration::from_millis(2000)).await;

    // Get reputation measurements so that we can check that the non-revealing node is no longer
    // being monitored later.
    let prev_reputation_measurements_by_node = network
        .nodes()
        .map(|n| (n.index(), n.reputation_query().get_measurements()))
        .collect::<HashMap<_, _>>();

    // Sleep for a few seconds so that the pinger has time to send pings.
    tokio::time::sleep(Duration::from_secs(6)).await;

    // Check that the non-revealing node is no longer being monitored while all others are still
    // being monitored.
    for from_node in network.nodes() {
        let from_node_index = from_node.index();

        let prev_reputation_measurements = &prev_reputation_measurements_by_node[&from_node_index];

        // Get the new reputation measurements.
        let new_reputation_measurements = from_node.reputation_query().get_measurements();

        for to_node in network.nodes() {
            let to_node_index = to_node.index();

            if from_node_index == to_node_index {
                continue;
            }

            let prev_measurements = prev_reputation_measurements[&to_node_index]
                .interactions
                .unwrap();
            let new_measurements = new_reputation_measurements[&to_node_index]
                .interactions
                .unwrap();
            if to_node_index == 4 {
                // The non-revealing node should have no new interactions.
                // We allow 2 new interactions in case it raced the removal of the node from the
                // pinger components.
                assert!(
                    new_measurements - prev_measurements <= 2
                        && new_measurements - prev_measurements >= 0,
                    "expected no new interactions for non-revealing node {} from node {} (prev: {:?}, new: {:?})",
                    to_node_index,
                    from_node_index,
                    prev_measurements,
                    new_measurements,
                );
            } else {
                // The other nodes should have new interactions.
                assert!(
                    new_measurements > prev_measurements,
                    "expected new interactions for node {} from node {} (prev: {:?}, new: {:?})",
                    to_node_index,
                    from_node_index,
                    prev_measurements,
                    new_measurements,
                );
            }
        }
    }

    // Shutdown the nodes.
    network.shutdown().await;
}

#[tokio::test]
async fn test_non_revealing_node_partially_slashed_sufficient_stake() {
    let commit_phase_duration = COMMIT_PHASE_DURATION;
    let reveal_phase_duration = REVEAL_PHASE_DURATION;
    let mut network = build_network(BuildNetworkOptions {
        committee_nodes: 4,
        committee_nodes_without_beacon: 1,
        // The node's initial stake is 1000, and it will be slashed 500, leaving sufficient stake
        // for a node.
        min_stake: 500,
        non_reveal_slash_amount: 500,
        commit_phase_duration,
        reveal_phase_duration,
        real_consensus: true,
        ping_interval: Some(Duration::from_secs(1)),
        ..Default::default()
    })
    .await
    .unwrap();

    wait_for_forwarder_to_start().await;

    // Execute epoch change transactions from all nodes.
    let epoch = network.node(0).app_query().get_current_epoch();
    network.change_epoch().await.unwrap();

    // Wait for the phase metadata to be set.
    // This should not be necessary but it seems that the data is not always immediately available
    // from the app state query runner after the block is executed.
    wait_for_committee_selection_beacon_phase(&network, |phase| {
        matches!(phase, Some(CommitteeSelectionBeaconPhase::Commit((0, 0))))
    })
    .await
    .unwrap();

    // Submit commit transaction from the node that will be non-revealing.
    network
        .node(4)
        .execute_transaction_from_node(UpdateMethod::CommitteeSelectionBeaconCommit {
            commit: [0; 32].into(),
        })
        .await
        .unwrap();

    // Wait for the commit phase to end, then submit the commit timeout txn.
    tokio::time::sleep(Duration::from_millis(commit_phase_duration)).await;
    // Since we use the real consensus, the commit phase timeout transaction will be send from the
    // consensus.

    // Wait to transition to the reveal phase.
    wait_for_committee_selection_beacon_phase(&network, |phase| {
        matches!(phase, Some(CommitteeSelectionBeaconPhase::Reveal((0, 0))))
    })
    .await
    .unwrap();

    // Wait for the reveal phase to end, then submit the reveal timeout txn.
    tokio::time::sleep(Duration::from_millis(reveal_phase_duration)).await;
    // Since we use the real consensus, the reveal phase timeout transaction will be send from the
    // consensus.

    // Check that we transition to a new commit phase after reveal phase timeout.
    wait_for_committee_selection_beacon_phase(&network, |phase| {
        matches!(phase, Some(CommitteeSelectionBeaconPhase::Commit((0, 1))))
    })
    .await
    .unwrap();

    // Get reputation measurements so that we can check that the non-revealing node is still being
    // monitored since still has sufficient stake to be active.
    let prev_reputation_measurements_by_node = network
        .nodes()
        .map(|n| (n.index(), n.reputation_query().get_measurements()))
        .collect::<HashMap<_, _>>();

    // Check that the epoch has not changed.
    for node in network.nodes() {
        assert_eq!(node.app_query().get_current_epoch(), epoch);
    }

    for node in network.nodes() {
        // Check that the non-revealing node has been slashed, and the other have not.
        assert_eq!(
            node.app_query()
                .get_node_info(&4, |n| n.stake.staked)
                .unwrap(),
            500u64.into()
        );
        assert_eq!(
            node.app_query()
                .get_node_info(&0, |n| n.stake.staked)
                .unwrap(),
            1000u64.into()
        );

        // Check that the active set includes all nodes.
        assert_eq!(
            node.app_query().get_committee_members_by_index(),
            vec![0, 1, 2, 3, 4]
        );
        let node_registry_changes = node
            .app_query()
            .get_committee_info(&epoch, |c| c.node_registry_changes)
            .unwrap();
        assert_eq!(
            *node_registry_changes.iter().last().unwrap().1,
            vec![(
                network.node(4).get_node_public_key(),
                NodeRegistryChange::Slashed((
                    500u64.into(),
                    Staking {
                        staked: 500u64.into(),
                        stake_locked_until: 0,
                        locked: 0u64.into(),
                        locked_until: 0,
                    },
                    NodeRegistryChangeSlashReason::CommitteeBeaconNonReveal,
                )),
            )]
        );
    }

    // Submit commit transaction from the non-revealing node and check that it's reverted.
    let receipt = network
        .node(4)
        .execute_transaction_from_node_with_receipt(
            UpdateMethod::CommitteeSelectionBeaconCommit {
                commit: [0; 32].into(),
            },
            Duration::from_secs(10),
        )
        .await
        .unwrap();

    assert!(matches!(
        receipt,
        TransactionReceipt {
            response: TransactionResponse::Revert(
                ExecutionError::CommitteeSelectionBeaconNonRevealingNode,
            ),
            ..
        },
    ));

    // Wait a few seconds so that the pinger has time to send pings.
    tokio::time::sleep(Duration::from_secs(6)).await;

    // Check that the non-revealing node is still being monitored, along with all other nodes.
    for from_node in network.nodes() {
        let from_node_index = from_node.index();

        let prev_reputation_measurements = &prev_reputation_measurements_by_node[&from_node_index];

        // Get the new reputation measurements.
        let new_reputation_measurements = from_node.reputation_query().get_measurements();

        for to_node in network.nodes() {
            let to_node_index = to_node.index();

            if from_node_index == to_node_index {
                continue;
            }

            // The other nodes should have new interactions.
            assert_ne!(
                new_reputation_measurements[&to_node_index].interactions,
                prev_reputation_measurements[&to_node_index].interactions,
                "expected new interactions for node {} from node {}",
                to_node_index,
                from_node_index,
            );
        }
    }

    // Shutdown the nodes.
    network.shutdown().await;
}

#[tokio::test]
async fn test_node_attempts_reveal_without_committment() {
    let commit_phase_duration = COMMIT_PHASE_DURATION;
    let reveal_phase_duration = REVEAL_PHASE_DURATION;
    let mut network = build_network(BuildNetworkOptions {
        committee_nodes: 3,
        committee_nodes_without_beacon: 1,
        commit_phase_duration,
        reveal_phase_duration,
        ..Default::default()
    })
    .await
    .unwrap();

    wait_for_forwarder_to_start().await;

    // Execute epoch change transactions from all nodes.
    let epoch = network.node(0).app_query().get_current_epoch();
    network.change_epoch().await.unwrap();

    wait_for_committee_selection_beacon_phase(&network, |phase| {
        matches!(phase, Some(CommitteeSelectionBeaconPhase::Commit((0, 0))))
    })
    .await
    .unwrap();

    // Node 2 does not have the committee-beacon component running, so it won't
    // send a commit during the commit phase.

    // Wait for the commit phase to end, then submit the commit timeout txn.
    tokio::time::sleep(Duration::from_millis(commit_phase_duration)).await;
    // Send commit phase timeout transaction from all nodes.
    network.commit_phase_timeout(0).await.unwrap();

    // Wait to transition to the reveal phase.
    wait_for_committee_selection_beacon_phase(&network, |phase| {
        matches!(phase, Some(CommitteeSelectionBeaconPhase::Reveal((0, 0))))
    })
    .await
    .unwrap();

    // Execute reveal transaction from the node that didn't commit.
    let receipt = network
        .node(3)
        .execute_transaction_from_node_with_receipt(
            UpdateMethod::CommitteeSelectionBeaconReveal { reveal: [1; 32] },
            Duration::from_secs(10),
        )
        .await
        .unwrap();

    assert!(matches!(
        receipt,
        TransactionReceipt {
            response: TransactionResponse::Revert(
                ExecutionError::CommitteeSelectionBeaconNotCommitted,
            ),
            ..
        },
    ));

    // Wait for the reveal phase to end, then submit the reveal timeout txn.
    tokio::time::sleep(Duration::from_millis(reveal_phase_duration)).await;
    network.reveal_phase_timeout(0).await.unwrap();

    // Wait for reveal phase to complete and beacon phase to be unset.
    wait_for_committee_selection_beacon_phase(&network, |phase| phase.is_none())
        .await
        .unwrap();

    // Check that the epoch has been incremented.
    for node in network.nodes() {
        assert_eq!(node.app_query().get_current_epoch(), epoch + 1);
    }

    // Check that the app state beacons are cleared.
    for node in network.nodes() {
        assert!(node
            .app_query()
            .get_committee_selection_beacons()
            .is_empty());
    }

    // Shutdown the nodes.
    network.shutdown().await;
}

#[derive(Debug, Clone)]
pub struct BuildNetworkOptions {
    pub committee_nodes: usize,
    pub committee_nodes_without_beacon: usize,
    pub commit_phase_duration: u64,
    pub reveal_phase_duration: u64,
    pub consensus_buffer_interval: Duration,
    pub min_stake: u64,
    pub non_reveal_slash_amount: u64,
    pub real_consensus: bool,
    pub ping_interval: Option<Duration>,
}

impl Default for BuildNetworkOptions {
    fn default() -> Self {
        Self {
            committee_nodes: 0,
            committee_nodes_without_beacon: 0,
            commit_phase_duration: 3,
            reveal_phase_duration: 3,
            consensus_buffer_interval: Duration::from_millis(200),
            min_stake: 1000,
            non_reveal_slash_amount: 1000,
            real_consensus: false,
            ping_interval: None,
        }
    }
}

async fn build_network(options: BuildNetworkOptions) -> Result<TestNetwork> {
    let committee_beacon_config = CommitteeBeaconConfig::default();

    let mut builder = TestNetwork::builder();

    if let Some(ping_interval) = options.ping_interval {
        builder = builder.with_ping_interval(ping_interval);
    }

    if options.real_consensus {
        builder = builder.with_real_consensus();
    } else {
        builder = builder.with_mock_consensus(MockConsensusConfig {
            max_ordering_time: 0,
            min_ordering_time: 0,
            probability_txn_lost: 0.0,
            new_block_interval: Duration::from_millis(0),
            transactions_to_lose: Default::default(),
            block_buffering_interval: options.consensus_buffer_interval,
            forwarder_transaction_to_error: Default::default(),
        })
    }

    builder = builder.with_committee_beacon_config(committee_beacon_config.clone());

    if options.real_consensus {
        builder = builder
            .with_committee_nodes::<TestFullNodeComponentsWithRealConsensus>(
                options.committee_nodes,
            )
            .await;
    } else {
        builder = builder
            .with_committee_nodes::<TestFullNodeComponentsWithMockConsensus>(
                options.committee_nodes,
            )
            .await;
    }

    builder = builder.with_genesis_mutator(move |genesis| {
        genesis.committee_selection_beacon_commit_phase_duration = options.commit_phase_duration;
        genesis.committee_selection_beacon_reveal_phase_duration = options.reveal_phase_duration;
        genesis.min_stake = options.min_stake;
        genesis.committee_selection_beacon_non_reveal_slash_amount =
            options.non_reveal_slash_amount;
    });

    let mut i = builder.nodes.len();
    if options.real_consensus {
        for _ in 0..options.committee_nodes_without_beacon {
            builder = builder.with_node(
                TestNodeBuilder::new()
                    .with_real_consensus()
                    .build::<TestFullNodeComponentsWithRealConsensusWithoutCommitteeBeacon>(Some(
                        format!("node-{}", i),
                    ))
                    .await?,
            );
            i += 1;
        }
    } else {
        let consensus_group = builder.mock_consensus_group();

        for _ in 0..options.committee_nodes_without_beacon {
            builder = builder.with_node(
                TestNodeBuilder::new()
                    .with_mock_consensus(consensus_group.clone())
                    .build::<TestFullNodeComponentsWithoutCommitteeBeacon>(Some(format!(
                        "node-{}",
                        i,
                    )))
                    .await?,
            );
            i += 1;
        }
    }

    builder.build().await
}

async fn wait_for_forwarder_to_start() {
    tokio::time::sleep(Duration::from_millis(2000)).await;
}

async fn wait_for_narwhal_restart() {
    tokio::time::sleep(Duration::from_millis(4000)).await;
}

/// Wait for committee selection beacon phase to satisfy the given predicate across all nodes, with
/// all values of all nodes being equal.
async fn wait_for_committee_selection_beacon_phase<F>(
    network: &TestNetwork,
    predicate: F,
) -> Result<Option<CommitteeSelectionBeaconPhase>, PollUntilError>
where
    F: Fn(&Option<CommitteeSelectionBeaconPhase>) -> bool,
{
    poll_until(
        || async {
            let phases: Vec<Option<CommitteeSelectionBeaconPhase>> = network
                .nodes()
                .map(|node| node.app_query().get_committee_selection_beacon_phase())
                .collect();

            if phases
                .iter()
                .all(|phase| predicate(phase) && phase == &phases[0])
            {
                Ok(phases[0].clone())
            } else {
                Err(PollUntilError::ConditionNotSatisfied)
            }
        },
        Duration::from_secs(30),
        Duration::from_millis(100),
    )
    .await
}
