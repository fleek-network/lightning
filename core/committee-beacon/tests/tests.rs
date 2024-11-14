use std::collections::{HashMap, HashSet};
use std::time::Duration;

use anyhow::Result;
use lightning_committee_beacon::{CommitteeBeaconConfig, CommitteeBeaconTimerConfig};
use lightning_interfaces::prelude::*;
use lightning_interfaces::types::UpdateMethod;
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
use tokio::time::Instant;
use types::{
    CommitteeSelectionBeaconCommit,
    CommitteeSelectionBeaconPhase,
    ExecuteTransactionError,
    ExecuteTransactionOptions,
    ExecuteTransactionRetry,
    ExecutionError,
    NodeIndex,
    NodeRegistryChange,
    NodeRegistryChangeSlashReason,
    Staking,
    TransactionReceipt,
    TransactionResponse,
};

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
    let mut network = build_network(BuildNetworkOptions {
        committee_nodes: 1,
        ..Default::default()
    })
    .await
    .unwrap();
    let node = network
        .node(0)
        .downcast::<TestFullNodeComponentsWithMockConsensus>();

    // Send epoch change transaction from all nodes.
    let epoch = network.change_epoch().await.unwrap();

    // Check that beacon phase is set.
    // We don't check for commit phase specifically because we can't be sure it hasn't transitioned
    // to the reveal phase before checking.
    wait_for_committee_selection_beacon_phase(&network, |phase| phase.is_some())
        .await
        .unwrap();

    // Wait for reveal phase to complete and beacon phase to be unset.
    wait_for_committee_selection_beacon_phase(&network, |phase| phase.is_none())
        .await
        .unwrap();

    // Check that the epoch has been incremented.
    let new_epoch = node.get_epoch();
    assert_eq!(new_epoch, epoch);

    // Check that the app state beacons are cleared.
    assert!(
        node.app_query()
            .get_committee_selection_beacons()
            .is_empty()
    );

    // Check that the local database beacons are eventually cleared.
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
    let mut network = build_network(BuildNetworkOptions {
        committee_nodes: 3,
        ..Default::default()
    })
    .await
    .unwrap();
    let node = network
        .node(0)
        .downcast::<TestFullNodeComponentsWithMockConsensus>();

    // Send epoch change transaction from all nodes.
    let epoch = network.change_epoch().await.unwrap();

    // Check that beacon phase is set.
    // We don't check for commit phase specifically because we can't be sure it hasn't transitioned
    // to the reveal phase before checking.
    wait_for_committee_selection_beacon_phase(&network, |phase| phase.is_some())
        .await
        .unwrap();

    // Wait for reveal phase to complete and beacon phase to be unset.
    wait_for_committee_selection_beacon_phase(&network, |phase| phase.is_none())
        .await
        .unwrap();

    // Check that the epoch has been incremented.
    for node in network.nodes() {
        assert_eq!(node.app_query().get_current_epoch(), epoch);
    }

    // Check that the app state beacons are cleared.
    for node in network.nodes() {
        assert!(
            node.app_query()
                .get_committee_selection_beacons()
                .is_empty()
        );
    }

    // Check that the local database beacons are eventually cleared.
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
async fn test_block_executed_in_waiting_phase_should_do_nothing() {
    let mut network = TestNetwork::builder()
        .with_committee_nodes::<TestFullNodeComponentsWithMockConsensus>(2)
        .await
        .build()
        .await
        .unwrap();
    let node = network
        .node(0)
        .downcast::<TestFullNodeComponentsWithMockConsensus>();
    let query = node.app_query();

    // Check beacon phase before submitting transaction.
    let phase = query.get_committee_selection_beacon_phase();
    assert!(phase.is_none());

    // Submit a transaction that does nothing except increment the node's nonce.
    network
        .node(0)
        .execute_transaction_from_node(UpdateMethod::IncrementNonce {}, None)
        .await
        .unwrap();

    // Check that beacon phase has not changed.
    let phase = query.get_committee_selection_beacon_phase();
    assert!(phase.is_none());

    // Check that there are no node beacons (commits and reveals) in app state.
    let beacons = query.get_committee_selection_beacons();
    assert!(beacons.is_empty());

    // Check that there are no beacons in our local database.
    let beacons = node.committee_beacon().query().get_beacons();
    assert!(beacons.is_empty());

    // Shutdown the network.
    network.shutdown().await;
}

#[tokio::test]
async fn test_insufficient_participation_in_commit_phase() {
    let mut network = build_network(BuildNetworkOptions {
        committee_nodes: 1,
        committee_nodes_without_beacon: 2,
        consensus_buffer_interval: Duration::from_millis(100),
        committee_beacon_timer_tick_delay: Duration::from_millis(100),
        ..Default::default()
    })
    .await
    .unwrap();

    // Execute epoch change transactions from all nodes.
    let epoch = network.node(0).app_query().get_current_epoch();
    network.change_epoch().await.unwrap();

    // Wait for the phase metadata to be set.
    // This should not be necessary but it seems that the data is not always immediately available
    // from the app state query runner after the block is executed.
    wait_for_committee_selection_beacon_phase(&network, |phase| phase.is_some())
        .await
        .unwrap();

    // Check that we stay in the commit phase, and that the block range and round advances.
    let start = Instant::now();
    let mut round_per_node = HashMap::new();
    let mut block_range_per_node = HashMap::new();
    while start.elapsed() < Duration::from_secs(3) {
        for node in network.nodes() {
            let round = *round_per_node.entry(node.index()).or_insert(0);
            let block_range = *block_range_per_node.entry(node.index()).or_insert((0, 0));

            let current_phase = node
                .app_query()
                .get_committee_selection_beacon_phase()
                .unwrap();
            let current_round = node
                .app_query()
                .get_committee_selection_beacon_round()
                .unwrap();

            // Check that we're in a commit phase.
            assert!(matches!(
                current_phase,
                CommitteeSelectionBeaconPhase::Commit(_)
            ));

            // Check that the block range advances.
            match current_phase {
                CommitteeSelectionBeaconPhase::Commit((start_block, end_block)) => {
                    assert!(
                        end_block - start_block
                            == network
                                .genesis
                                .committee_selection_beacon_commit_phase_duration
                    );
                    if block_range != (start_block, end_block) {
                        assert!(start_block > block_range.0 && end_block > block_range.1);
                        block_range_per_node.insert(node.index(), (start_block, end_block));
                    }
                },
                _ => unreachable!(),
            }

            // Check that the round advances.
            if round == 0 {
                round_per_node.insert(node.index(), current_round);
            } else if current_round != round {
                assert_eq!(current_round, round + 1);
                round_per_node.insert(node.index(), current_round);
            }
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    for node in network.nodes() {
        let round = *round_per_node.get(&node.index()).unwrap();
        assert!(round > 1);

        let block_range = *block_range_per_node.get(&node.index()).unwrap();
        assert!(
            block_range.1 > 0
                && block_range.1
                    > network
                        .genesis
                        .committee_selection_beacon_reveal_phase_duration
        );
    }

    // Check that the epoch has not changed.
    for node in network.nodes() {
        assert_eq!(node.app_query().get_current_epoch(), epoch);
    }

    // Shutdown the nodes.
    network.shutdown().await;
}

#[tokio::test]
async fn test_single_revealing_node_fully_slashed() {
    let mut network = build_network(BuildNetworkOptions {
        real_consensus: true,
        committee_nodes: 4,
        committee_nodes_without_beacon: 1,
        // The node's initial stake is 1000, and it will be slashed 1000, leaving insufficient stake
        // for a node.
        commit_phase_duration: 10,
        reveal_phase_duration: 10,
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

    // Execute epoch change transactions from all nodes.
    let epoch = network.node(0).app_query().get_current_epoch();
    network.change_epoch().await.unwrap();

    // Wait for the phase metadata to be set.
    // This should not be necessary but it seems that the data is not always immediately available
    // from the app state query runner after the block is executed.
    wait_for_committee_selection_beacon_phase(&network, |phase| phase.is_some())
        .await
        .unwrap();

    // Submit commit transaction from the node that will be non-revealing.
    // We do this manually because the node does not have a committee beacon component running,
    // where all other nodes do.
    let non_revealing_node = network.node(4);
    non_revealing_node
        .execute_transaction_from_node(
            UpdateMethod::CommitteeSelectionBeaconCommit {
                commit: [0; 32].into(),
            },
            None,
        )
        .await
        .unwrap();

    // Get the current round.
    let round = network
        .node(0)
        .app_query()
        .get_committee_selection_beacon_round()
        .unwrap();

    // Wait to transition to the reveal phase.
    wait_for_committee_selection_beacon_phase(&network, |phase| {
        matches!(phase, Some(CommitteeSelectionBeaconPhase::Reveal(_)))
    })
    .await
    .unwrap();

    // Check that we transition to a new commit phase after reveal phase timeout.
    wait_for_committee_selection_beacon_phase(&network, |phase| {
        matches!(phase, Some(CommitteeSelectionBeaconPhase::Commit(_)))
    })
    .await
    .unwrap();

    // Get reputation measurements so that we can check that the non-revealing node is no longer
    // being monitored later.
    let prev_reputation_measurements_by_node = network
        .nodes()
        .map(|n| (n.index(), n.reputation_query().get_measurements()))
        .collect::<HashMap<_, _>>();

    for node in network.nodes() {
        // Check that we are in a new round.
        assert_eq!(
            node.app_query().get_committee_selection_beacon_round(),
            Some(round + 1)
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

    // Submit commit transaction from the non-revealing node and check that it's reverted.
    let error = non_revealing_node
        .execute_transaction_from_node(
            UpdateMethod::CommitteeSelectionBeaconCommit {
                commit: [0; 32].into(),
            },
            None,
        )
        .await
        .unwrap_err();
    assert!(
        matches!(
            error,
            ExecuteTransactionError::Reverted((
                _,
                TransactionReceipt {
                    response: TransactionResponse::Revert(ExecutionError::InsufficientStake),
                    ..
                },
                _
            ))
        ),
        "{}",
        error
    );

    // Sleep for a few seconds so that the pinger has time to send pings.
    tokio::time::sleep(Duration::from_secs(3)).await;

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

            if to_node_index == 4 {
                // The non-revealing node should have no new interactions.
                assert_eq!(
                    new_reputation_measurements[&to_node_index].interactions,
                    prev_reputation_measurements[&to_node_index].interactions,
                    "expected no interactions for non-revealing node {} from node {}",
                    to_node_index,
                    from_node_index,
                );
            } else {
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
    }

    // Shutdown the nodes.
    network.shutdown().await;
}

#[tokio::test]
async fn test_non_revealing_node_partially_slashed_insufficient_stake() {
    let mut network = build_network(BuildNetworkOptions {
        committee_nodes: 4,
        committee_nodes_without_beacon: 1,
        // The node's initial stake is 1000, and it will be slashed 500, leaving insufficient stake
        // for a node.
        min_stake: 1000,
        non_reveal_slash_amount: 500,
        commit_phase_duration: 10,
        reveal_phase_duration: 10,
        real_consensus: true,
        ping_interval: Some(Duration::from_secs(1)),
        ..Default::default()
    })
    .await
    .unwrap();

    // Execute epoch change transactions from all nodes.
    let epoch = network.node(0).app_query().get_current_epoch();
    network.change_epoch().await.unwrap();

    // Wait for the phase metadata to be set.
    // This should not be necessary but it seems that the data is not always immediately available
    // from the app state query runner after the block is executed.
    wait_for_committee_selection_beacon_phase(&network, |phase| phase.is_some())
        .await
        .unwrap();

    // Submit commit transaction from the node that will be non-revealing.
    network
        .node(4)
        .execute_transaction_from_node(
            UpdateMethod::CommitteeSelectionBeaconCommit {
                commit: [0; 32].into(),
            },
            None,
        )
        .await
        .unwrap();

    // Get the current round.
    let round = network
        .node(0)
        .app_query()
        .get_committee_selection_beacon_round()
        .unwrap();

    // Wait to transition to the reveal phase.
    wait_for_committee_selection_beacon_phase(&network, |phase| {
        matches!(phase, Some(CommitteeSelectionBeaconPhase::Reveal(_)))
    })
    .await
    .unwrap();

    // Check that we transition to a new commit phase after the reveal phase timeout.
    wait_for_committee_selection_beacon_phase(&network, |phase| {
        matches!(phase, Some(CommitteeSelectionBeaconPhase::Commit(_)))
    })
    .await
    .unwrap();

    // Get reputation measurements so that we can check that the non-revealing node is no longer
    // being monitored later.
    let prev_reputation_measurements_by_node = network
        .nodes()
        .map(|n| (n.index(), n.reputation_query().get_measurements()))
        .collect::<HashMap<_, _>>();

    // Check that we are in a new round.
    assert_eq!(
        network
            .node(0)
            .app_query()
            .get_committee_selection_beacon_round(),
        Some(round + 1)
    );

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

    // Submit commit transaction from the non-revealing node and check that it's reverted.
    let result = network
        .node(4)
        .execute_transaction_from_node(
            UpdateMethod::CommitteeSelectionBeaconCommit {
                commit: [0; 32].into(),
            },
            None,
        )
        .await;
    assert!(matches!(
        result,
        Err(ExecuteTransactionError::Reverted((
            _,
            TransactionReceipt {
                response: TransactionResponse::Revert(ExecutionError::InsufficientStake),
                ..
            },
            _
        )))
    ));

    // Sleep for a few seconds so that the pinger has time to send pings.
    tokio::time::sleep(Duration::from_secs(3)).await;

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

            if to_node_index == 4 {
                // The non-revealing node should have no new interactions.
                assert_eq!(
                    new_reputation_measurements[&to_node_index].interactions,
                    prev_reputation_measurements[&to_node_index].interactions,
                    "expected no interactions for non-revealing node {} from node {}",
                    to_node_index,
                    from_node_index,
                );
            } else {
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
    }

    // Shutdown the nodes.
    network.shutdown().await;
}

#[tokio::test]
async fn test_non_revealing_node_partially_slashed_sufficient_stake() {
    let mut network = build_network(BuildNetworkOptions {
        committee_nodes: 4,
        committee_nodes_without_beacon: 1,
        // The node's initial stake is 1000, and it will be slashed 500, leaving sufficient stake
        // for a node.
        min_stake: 500,
        non_reveal_slash_amount: 500,
        commit_phase_duration: 10,
        reveal_phase_duration: 10,
        real_consensus: true,
        ping_interval: Some(Duration::from_secs(1)),
        ..Default::default()
    })
    .await
    .unwrap();

    // Execute epoch change transactions from all nodes.
    let epoch = network.node(0).app_query().get_current_epoch();
    network.change_epoch().await.unwrap();

    // Wait for the phase metadata to be set.
    // This should not be necessary but it seems that the data is not always immediately available
    // from the app state query runner after the block is executed.
    wait_for_committee_selection_beacon_phase(&network, |phase| phase.is_some())
        .await
        .unwrap();

    // Submit commit transaction from the node that will be non-revealing.
    network
        .node(4)
        .execute_transaction_from_node(
            UpdateMethod::CommitteeSelectionBeaconCommit {
                commit: [0; 32].into(),
            },
            None,
        )
        .await
        .unwrap();

    // Get the current round.
    let round = network
        .node(0)
        .app_query()
        .get_committee_selection_beacon_round()
        .unwrap();

    // Wait to transition to the reveal phase.
    wait_for_committee_selection_beacon_phase(&network, |phase| {
        matches!(phase, Some(CommitteeSelectionBeaconPhase::Reveal(_)))
    })
    .await
    .unwrap();

    // Check that we transition to a new commit phase.
    wait_for_committee_selection_beacon_phase(&network, |phase| {
        matches!(phase, Some(CommitteeSelectionBeaconPhase::Commit(_)))
    })
    .await
    .unwrap();

    // Get reputation measurements so that we can check that the non-revealing node is still being
    // monitored since still has sufficient stake to be active.
    let prev_reputation_measurements_by_node = network
        .nodes()
        .map(|n| (n.index(), n.reputation_query().get_measurements()))
        .collect::<HashMap<_, _>>();

    // Check that we are in a new round.
    assert_eq!(
        network
            .node(0)
            .app_query()
            .get_committee_selection_beacon_round(),
        Some(round + 1)
    );

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
    let result = network
        .node(4)
        .execute_transaction_from_node(
            UpdateMethod::CommitteeSelectionBeaconCommit {
                commit: [0; 32].into(),
            },
            None,
        )
        .await;
    assert!(matches!(
        result,
        Err(ExecuteTransactionError::Reverted((
            _,
            TransactionReceipt {
                response: TransactionResponse::Revert(
                    ExecutionError::CommitteeSelectionBeaconNonRevealingNode
                ),
                ..
            },
            _
        )))
    ));

    // Wait a few seconds so that the pinger has time to send pings.
    tokio::time::sleep(Duration::from_secs(3)).await;

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
    let mut network = build_network(BuildNetworkOptions {
        committee_nodes: 2,
        committee_nodes_without_beacon: 2,
        ..Default::default()
    })
    .await
    .unwrap();

    // Execute epoch change transactions from all nodes.
    let epoch = network.node(0).app_query().get_current_epoch();
    network.change_epoch().await.unwrap();

    // Execute commit transaction from one of the nodes without a beacon component running.
    network
        .node(2)
        .execute_transaction_from_node(
            UpdateMethod::CommitteeSelectionBeaconCommit {
                commit: CommitteeSelectionBeaconCommit::build(epoch, 0, [2; 32]),
            },
            Some(ExecuteTransactionOptions {
                retry: ExecuteTransactionRetry::Never,
                wait: types::ExecuteTransactionWait::Receipt,
                ..Default::default()
            }),
        )
        .await
        .unwrap();

    // Wait to transition to the reveal phase.
    wait_for_committee_selection_beacon_phase(&network, |phase| {
        matches!(phase, Some(CommitteeSelectionBeaconPhase::Reveal(_)))
    })
    .await
    .unwrap();

    // Execute reveal transaction from the node that didn't commit.
    let result = network
        .node(3)
        .execute_transaction_from_node(
            UpdateMethod::CommitteeSelectionBeaconReveal { reveal: [1; 32] },
            Some(ExecuteTransactionOptions {
                retry: ExecuteTransactionRetry::Never,
                wait: types::ExecuteTransactionWait::Receipt,
                ..Default::default()
            }),
        )
        .await;

    // Check that the reveal transaction was reverted.
    assert!(matches!(
        result,
        Err(ExecuteTransactionError::Reverted((
            _,
            TransactionReceipt {
                response: TransactionResponse::Revert(
                    ExecutionError::CommitteeSelectionBeaconNotCommitted,
                ),
                ..
            },
            _
        )))
    ));

    // Execute reveal transaction from the node that did commit so that the epoch change advances.
    network
        .node(2)
        .execute_transaction_from_node(
            UpdateMethod::CommitteeSelectionBeaconReveal { reveal: [2; 32] },
            Some(ExecuteTransactionOptions {
                retry: ExecuteTransactionRetry::Never,
                wait: types::ExecuteTransactionWait::Receipt,
                ..Default::default()
            }),
        )
        .await
        .unwrap();

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
        assert!(
            node.app_query()
                .get_committee_selection_beacons()
                .is_empty()
        );
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
    pub committee_beacon_timer_tick_delay: Duration,
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
            committee_beacon_timer_tick_delay: Duration::from_millis(200),
            consensus_buffer_interval: Duration::from_millis(200),
            min_stake: 1000,
            non_reveal_slash_amount: 1000,
            real_consensus: false,
            ping_interval: None,
        }
    }
}

async fn build_network(options: BuildNetworkOptions) -> Result<TestNetwork> {
    let committee_beacon_config = CommitteeBeaconConfig {
        timer: CommitteeBeaconTimerConfig {
            tick_delay: options.committee_beacon_timer_tick_delay,
        },
        ..Default::default()
    };

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
        Duration::from_secs(15),
        Duration::from_millis(100),
    )
    .await
}
