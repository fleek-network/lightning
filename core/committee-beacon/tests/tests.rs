use std::time::Duration;

use anyhow::Result;
use futures::future::join_all;
use lightning_committee_beacon::{CommitteeBeaconConfig, CommitteeBeaconTimerConfig};
use lightning_interfaces::prelude::*;
use lightning_interfaces::types::UpdateMethod;
use lightning_interfaces::{
    CommitteeBeaconInterface,
    CommitteeBeaconQueryInterface,
    SyncQueryRunnerInterface,
};
use lightning_test_utils::consensus::Config as MockConsensusConfig;
use lightning_test_utils::e2e::{
    DowncastToTestFullNode,
    TestFullNodeComponentsWithMockConsensus,
    TestFullNodeComponentsWithoutCommitteeBeacon,
    TestNetwork,
    TestNodeBuilder,
};
use lightning_utils::application::QueryRunnerExt;
use lightning_utils::poll::{poll_until, PollUntilError};
use tokio::time::Instant;
use types::{CommitteeSelectionBeaconPhase, ExecuteTransactionOptions, ExecuteTransactionWait};

#[tokio::test]
async fn test_start_shutdown() {
    let node = lightning_test_utils::e2e::TestNodeBuilder::new()
        .build::<TestFullNodeComponentsWithMockConsensus>()
        .await
        .unwrap();
    node.shutdown().await;
}

#[tokio::test]
async fn test_epoch_change_single_node() {
    let mut network = TestNetwork::builder()
        .with_committee_nodes::<TestFullNodeComponentsWithMockConsensus>(1)
        .await
        .build()
        .await
        .unwrap();
    let node = network
        .node(0)
        .downcast::<TestFullNodeComponentsWithMockConsensus>();

    // Send epoch change transaction from all nodes.
    let epoch = network.change_epoch().await;

    // Check that beacon phase is set.
    // We don't check for commit phase specifically because we can't be sure it hasn't transitioned
    // to the reveal phase before checking.
    poll_until(
        || async {
            node.get_committee_selection_beacon_phase()
                .is_some()
                .then_some(())
                .ok_or(PollUntilError::ConditionNotSatisfied)
        },
        Duration::from_secs(3),
        Duration::from_millis(100),
    )
    .await
    .unwrap();

    // Check that beacons are in app state.
    // These difficult to catch this at the right time with queries, so we just check that the
    // number is less than or equal to the number of nodes.
    let beacons = node.app_query().get_committee_selection_beacons();
    assert!(beacons.len() <= network.node_count());

    // Check that beacons are in local database.
    // These difficult to catch this at the right time with queries, so we just check that the
    // number is less than or equal to the number of nodes.
    let beacons = node.committee_beacon().query().get_beacons();
    assert!(beacons.len() <= network.node_count());

    // Wait for reveal phase to complete and beacon phase to be unset.
    network
        .wait_for_committee_selection_beacon_phase_unset()
        .await
        .unwrap();

    // Check that the epoch has been incremented.
    let new_epoch = node.get_epoch();
    assert_eq!(new_epoch, epoch);

    // Check that there are no node beacons (commits and reveals) in app state.
    let beacons = node.app_query().get_committee_selection_beacons();
    assert!(beacons.is_empty());

    // Clearing the beacons at epoch change is best-effort, since we can't guarantee that
    // the notification will be received or the listener will be running, in the case of a
    // deployment for example. This is fine, since the beacons will be cleared on the next
    // committee selection phase anyway, and we don't rely on it for correctness.
    let beacons = node.committee_beacon().query().get_beacons();
    assert!(beacons.len() <= network.node_count());

    // Shutdown the network.
    network.shutdown().await;
}

#[tokio::test]
async fn test_epoch_change_multiple_nodes() {
    let mut network = TestNetwork::builder()
        .with_committee_nodes::<TestFullNodeComponentsWithMockConsensus>(3)
        .await
        .build()
        .await
        .unwrap();
    let node = network
        .node(0)
        .downcast::<TestFullNodeComponentsWithMockConsensus>();

    // Send epoch change transaction from all nodes.
    let epoch = network.change_epoch().await;

    // Check that beacon phase is set.
    // We don't check for commit phase specifically because we can't be sure it hasn't transitioned
    // to the reveal phase before checking.
    poll_until(
        || async {
            node.get_committee_selection_beacon_phase()
                .is_some()
                .then_some(())
                .ok_or(PollUntilError::ConditionNotSatisfied)
        },
        Duration::from_secs(3),
        Duration::from_millis(100),
    )
    .await
    .unwrap();

    // Check that beacons are in app state.
    // It's difficult to catch this at the right time with queries, so we just check that the
    // number is less than or equal to the number of nodes.
    let beacons = node.app_query().get_committee_selection_beacons();
    assert!(beacons.len() <= network.node_count());

    // Check that beacons are in local database.
    // It's difficult to catch this at the right time with queries, so we just check that the
    // number is less than or equal to the number of nodes.
    let beacons = node.committee_beacon().query().get_beacons();
    assert!(beacons.len() <= network.node_count());

    // Wait for reveal phase to complete and beacon phase to be unset.
    network
        .wait_for_committee_selection_beacon_phase_unset()
        .await
        .unwrap();

    // Check that the epoch has been incremented.
    let new_epoch = node.get_epoch();
    assert_eq!(new_epoch, epoch);

    // Check that there are no node beacons (commits and reveals) in app state.
    let beacons = node.app_query().get_committee_selection_beacons();
    assert!(beacons.is_empty());

    // Clearing the beacons at epoch change is best-effort, since we can't guarantee that
    // the notification will be received or the listener will be running, in the case of a
    // deployment for example. This is fine, since the beacons will be cleared on the next
    // committee selection phase anyway, and we don't rely on it for correctness.
    let beacons = node.committee_beacon().query().get_beacons();
    assert!(beacons.len() <= network.node_count());

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
    let committee_beacon_config = CommitteeBeaconConfig {
        timer: CommitteeBeaconTimerConfig {
            tick_delay: Duration::from_millis(100),
        },
        ..Default::default()
    };
    let builder = TestNetwork::builder()
        .with_mock_consensus(MockConsensusConfig {
            max_ordering_time: 0,
            min_ordering_time: 0,
            probability_txn_lost: 0.0,
            new_block_interval: Duration::from_secs(0),
            transactions_to_lose: Default::default(),
        })
        .with_committee_beacon_config(committee_beacon_config.clone())
        .with_committee_nodes::<TestFullNodeComponentsWithMockConsensus>(1)
        .await
        .with_genesis_mutator(|genesis| {
            genesis.committee_selection_beacon_commit_phase_duration = 3;
            genesis.committee_selection_beacon_reveal_phase_duration = 3;
        });
    let consensus_group = builder.mock_consensus_group();
    let mut network = builder
        .with_node(
            TestNodeBuilder::new()
                .with_mock_consensus(consensus_group.clone())
                .with_committee_beacon_config(committee_beacon_config.clone())
                .build::<TestFullNodeComponentsWithoutCommitteeBeacon>()
                .await
                .unwrap(),
        )
        .with_node(
            TestNodeBuilder::new()
                .with_mock_consensus(consensus_group.clone())
                .with_committee_beacon_config(committee_beacon_config.clone())
                .build::<TestFullNodeComponentsWithoutCommitteeBeacon>()
                .await
                .unwrap(),
        )
        .build()
        .await
        .unwrap();

    // Execute epoch change transactions from all nodes.
    let epoch = 0;
    join_all(network.nodes().map(|node| async {
        node.execute_transaction_from_node(
            UpdateMethod::ChangeEpoch { epoch },
            Some(ExecuteTransactionOptions {
                wait: ExecuteTransactionWait::Receipt,
                ..Default::default()
            }),
        )
        .await
    }))
    .await
    .into_iter()
    .collect::<Result<Vec<_>, _>>()
    .unwrap();

    // Wait for the phase metadata to be set.
    // This should not be necessary but it seems that the data is not always immediately available
    // from the app state query runner after the block is executed.
    poll_until(
        || async {
            for node in network.nodes() {
                let phase = node.app_query().get_committee_selection_beacon_phase();
                if phase.is_none() {
                    tracing::debug!("phase is none (node {})", node.index());
                    return Err(PollUntilError::ConditionNotSatisfied);
                }
            }
            Ok(())
        },
        Duration::from_secs(3),
        Duration::from_millis(100),
    )
    .await
    .unwrap();

    // Check that we stay in the commit phase, and that the block range and round advances.
    let start = Instant::now();
    let mut round = 0;
    let mut block_range = (0, 0);
    while start.elapsed() < Duration::from_secs(5) {
        for node in network.nodes() {
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
                CommitteeSelectionBeaconPhase::Commit((start, end)) => {
                    assert!(
                        end - start
                            == network
                                .genesis
                                .committee_selection_beacon_commit_phase_duration
                    );
                    if block_range != (start, end) {
                        assert!(start > block_range.0 && end > block_range.1);
                        block_range = (start, end);
                    }
                },
                _ => unreachable!(),
            }

            // Check that the round advances.
            if round == 0 {
                round = current_round;
            } else if current_round != round {
                assert_eq!(current_round, round + 1);
                round += 1;
            }
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    assert!(round > 1);
    assert!(
        block_range.1 > 0
            && block_range.1
                > network
                    .genesis
                    .committee_selection_beacon_reveal_phase_duration
    );

    // Check that the epoch has not changed.
    for node in network.nodes() {
        assert_eq!(node.app_query().get_current_epoch(), epoch);
    }

    // Shutdown the nodes.
    network.shutdown().await;
}

#[tokio::test]
async fn test_insufficient_participation_in_reveal_phase() {
    // TODO(snormore): Implement this test.
}

#[tokio::test]
async fn test_node_attempts_reveal_without_committment() {
    // TODO(snormore): Implement this test.
}

#[tokio::test]
async fn test_invalid_reveal_mismatch_with_commit() {
    // TODO(snormore): Implement this test.
}

#[tokio::test]
async fn test_node_submits_commit_outside_of_commit_phase() {
    // TODO(snormore): Implement this test.
}

#[tokio::test]
async fn test_node_submits_reveal_outside_of_reveal_phase() {
    // TODO(snormore): Implement this test.
}

#[tokio::test]
async fn test_node_reuses_old_commitment() {
    // TODO(snormore): Implement this test.
}

#[tokio::test]
async fn test_node_reuses_old_reveal() {
    // TODO(snormore): Implement this test.
}

#[tokio::test]
async fn test_non_committee_node_participation() {
    // TODO(snormore): Implement this test.

    // TODO(snormore): Check that the next commmittee was selected.
}

#[tokio::test]
async fn test_malformed_commit() {
    // TODO(snormore): Implement this test.
}

#[tokio::test]
async fn test_malformed_reveal() {
    // TODO(snormore): Implement this test.
}

#[tokio::test]
async fn test_non_revealing_node_partially_slashed() {
    // TODO(snormore): Implement this test.

    // Check that the node was slashed.

    // Check that if the node attempts to commit in the next round, it will be rejected.

    // Check that the node is not included in sufficient participation for the next round.
}

#[tokio::test]
async fn test_non_revealing_node_fully_slashed() {
    // TODO(snormore): Implement this test.
}

#[tokio::test]
async fn test_high_volume_participation() {
    // TODO(snormore): Implement this test.
}

#[tokio::test]
async fn test_network_delays() {
    // TODO(snormore): Implement this test.
}

#[tokio::test]
async fn test_node_attempts_to_submit_reveal_during_commit_phase() {
    // TODO(snormore): Implement this test.
}

#[tokio::test]
async fn test_multiple_non_revealing_nodes() {
    // TODO(snormore): Implement this test.
}
