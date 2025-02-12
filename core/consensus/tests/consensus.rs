use std::time::Duration;

use futures::future::join_all;
use lightning_interfaces::prelude::*;
use lightning_test_utils::e2e::{TestFullNodeComponentsWithRealConsensus, TestNetwork};
use lightning_utils::application::QueryRunnerExt;
use lightning_utils::poll::{poll_until, PollUntilError};
use types::{
    ExecuteTransactionError,
    ExecuteTransactionOptions,
    ExecutionData,
    ForwarderError,
    TransactionResponse,
    UpdateMethod,
};

#[tokio::test]
async fn test_insufficient_nodes_in_committee() {
    let mut network = TestNetwork::builder()
        .with_real_consensus()
        // We need at least 2 nodes in the committee or else transactions will not execute.
        .with_committee_nodes::<TestFullNodeComponentsWithRealConsensus>(1)
        .await
        .build()
        .await
        .unwrap();

    // Attempt to execute an increment nonce transaction from the node.
    let result = network
        .node(0)
        .execute_transaction_from_node(
            UpdateMethod::IncrementNonce {},
            Some(ExecuteTransactionOptions {
                wait: types::ExecuteTransactionWait::Receipt,
                ..Default::default()
            }),
        )
        .await;
    match result.unwrap_err() {
        ExecuteTransactionError::ForwarderError((
            _,
            ForwarderError::FailedToSendToAnyConnection | ForwarderError::NoActiveConnections,
        )) => (),
        error => panic!(
            "expected ForwarderError::FailedToSendToAnyConnection error, got {:?}",
            error
        ),
    }

    // Shutdown the network.
    network.shutdown().await;
}

#[tokio::test]
async fn test_execute_transaction_as_committee_node() {
    let mut network = TestNetwork::builder()
        .with_real_consensus()
        .with_committee_nodes::<TestFullNodeComponentsWithRealConsensus>(4)
        .await
        .build()
        .await
        .unwrap();

    // Execute an increment nonce transaction from the first node.
    let (_, receipt) = network
        .node(0)
        .execute_transaction_from_node(
            UpdateMethod::IncrementNonce {},
            Some(ExecuteTransactionOptions {
                wait: types::ExecuteTransactionWait::Receipt,
                ..Default::default()
            }),
        )
        .await
        .unwrap();
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
                    let nonce = node
                        .app_query()
                        .get_node_info(&0, |node| node.nonce)
                        .unwrap();
                    // When transactions are submitted immediately after startup, they may fail to
                    // initially make it to the mempool, in which case it will timeout and be
                    // retried, with a backfill of the first nonce. So we need to check for a range
                    // of nonces (in case of retry).
                    nonce > 0 && nonce < 5
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
async fn test_execute_transaction_as_non_committee_node() {
    let mut network = TestNetwork::builder()
        .with_real_consensus()
        .with_committee_nodes::<TestFullNodeComponentsWithRealConsensus>(4)
        .await
        .with_non_committee_nodes::<TestFullNodeComponentsWithRealConsensus>(1)
        .await
        .build()
        .await
        .unwrap();

    // Execute an increment nonce transaction from the non-committee node.
    let non_committee_node = network.non_committee_nodes()[0];
    let (_, receipt) = non_committee_node
        .execute_transaction_from_node(
            UpdateMethod::IncrementNonce {},
            Some(ExecuteTransactionOptions {
                wait: types::ExecuteTransactionWait::Receipt,
                ..Default::default()
            }),
        )
        .await
        .unwrap();
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
                    let nonce = node
                        .app_query()
                        .get_node_info(&non_committee_node.index(), |node| node.nonce)
                        .unwrap();
                    // When transactions are submitted immediately after startup, they may fail to
                    // initially make it to the mempool, in which case it will timeout and be
                    // retried, with a backfill of the first nonce. So we need to check for a range
                    // of nonces (in case of retry).
                    nonce > 0 && nonce < 5
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
async fn test_epoch_change_via_time() {
    let mut network = TestNetwork::builder()
        .with_real_consensus()
        .with_genesis_mutator(|genesis| {
            // Trigger epoch change on startup.
            genesis.epoch_start = 0;
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

#[tokio::test]
async fn test_epoch_change_via_transactions() {
    let mut network = TestNetwork::builder()
        .with_real_consensus()
        .with_committee_nodes::<TestFullNodeComponentsWithRealConsensus>(4)
        .await
        .build()
        .await
        .unwrap();

    // Check that the current epoch is 0 across the network.
    for node in network.nodes() {
        assert_eq!(node.app_query().get_current_epoch(), 0);
    }

    // Execute change epoch transactions from 2/3+1 of the committee nodes.
    join_all(network.nodes().take(3).map(|node| {
        node.execute_transaction_from_node(
            UpdateMethod::ChangeEpoch { epoch: 0 },
            Some(ExecuteTransactionOptions {
                wait: types::ExecuteTransactionWait::Receipt,
                ..Default::default()
            }),
        )
    }))
    .await
    .into_iter()
    .collect::<Result<Vec<_>, _>>()
    .unwrap();

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
