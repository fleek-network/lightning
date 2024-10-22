use std::time::Duration;

use lightning_interfaces::prelude::*;
use lightning_test_utils::e2e::{TestFullNodeComponentsWithRealConsensus, TestNetwork};
use lightning_utils::application::QueryRunnerExt;
use lightning_utils::poll::{poll_until, PollUntilError};
use types::{ExecutionData, TransactionResponse, UpdateMethod};

#[tokio::test]
async fn test_execute_transaction() {
    let mut network = TestNetwork::builder()
        .with_real_consensus()
        .with_committee_nodes::<TestFullNodeComponentsWithRealConsensus>(4)
        .await
        .build()
        .await
        .unwrap();

    // Execute an `IncrementNonce` transaction from the first node.
    let (_, receipt) = network
        .node(0)
        .execute_transaction_from_node(UpdateMethod::IncrementNonce {}, None)
        .await
        .unwrap()
        .as_receipt();
    assert_eq!(
        receipt.response,
        TransactionResponse::Success(ExecutionData::None)
    );

    // Check that the node nonce was incremented across the network.
    poll_until(
        || async {
            network
                .nodes()
                .all(|node| {
                    let nonce = node.app_query().get_node_info(&0, |node| node.nonce);
                    // When transactions are submitted immediately after startup, they may fail to
                    // initially make it to the mempool, in which case it will timeout and be
                    // retried, with a backfill of the first nonce. So we need to check for either
                    // nonce 1 or 2 (in case of retry).
                    nonce == Some(1) || nonce == Some(2)
                })
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
async fn test_epoch_change() {
    let mut network = TestNetwork::builder()
        .with_real_consensus()
        .with_genesis_mutator(|genesis| {
            genesis.epoch_start = 0;
            genesis.epoch_time = 120000;
        })
        .with_committee_nodes::<TestFullNodeComponentsWithRealConsensus>(4)
        .await
        .build()
        .await
        .unwrap();

    // Check that the current epoch is 0 across the network.
    for node in network.nodes() {
        assert_eq!(node.app_query().get_current_epoch(), 0);
    }

    // Wait for epoch to be incremented across the network.
    poll_until(
        || async {
            network
                .nodes()
                .all(|node| node.app_query().get_current_epoch() == 1)
                .then_some(())
                .ok_or(PollUntilError::ConditionNotSatisfied)
        },
        Duration::from_secs(20),
        Duration::from_millis(100),
    )
    .await
    .unwrap();

    // Shutdown the network.
    network.shutdown().await;
}
