use std::collections::HashMap;
use std::time::Duration;

use anyhow::Result;
use atomo::{DefaultSerdeBackend, SerdeBackend};
use bit_set::BitSet;
use fleek_crypto::{ConsensusSignature, PublicKey, SecretKey};
use lightning_interfaces::types::{
    AggregateCheckpoint,
    CheckpointAttestation,
    Epoch,
    NodeIndex,
    Topic,
};
use lightning_interfaces::{
    BroadcastInterface,
    CheckpointerInterface,
    CheckpointerQueryInterface,
    KeystoreInterface,
    PubSub,
};
use lightning_test_utils::e2e::{
    DowncastToTestFullNode,
    TestFullNodeComponentsWithMockConsensus,
    TestNetwork,
    TestNodeBuilder,
};
use lightning_test_utils::keys::EphemeralKeystore;
use lightning_utils::poll::{poll_until, PollUntilError};
use pretty_assertions::assert_eq;

use crate::CheckpointBroadcastMessage;

#[tokio::test]
async fn test_start_shutdown() {
    let node = TestNodeBuilder::new()
        .build::<TestFullNodeComponentsWithMockConsensus>(None)
        .await
        .unwrap();
    node.shutdown().await;
}

#[tokio::test]
async fn test_supermajority_over_epoch_changes() {
    let mut network = TestNetwork::builder()
        .with_committee_nodes::<TestFullNodeComponentsWithMockConsensus>(3)
        .await
        .build()
        .await
        .unwrap();

    for epoch in 0..10 {
        // Emit epoch changed notification to all nodes.
        network
            .notify_epoch_changed(epoch, [2; 32].into(), [3; 32].into(), [1; 32])
            .await;

        // Check that the nodes have received and stored the checkpoint attestations.
        let headers_by_node =
            wait_for_checkpoint_attestations(&network, epoch, |headers_by_node| {
                headers_by_node
                    .values()
                    .map(|headers| headers.len())
                    .collect::<Vec<_>>()
                    == vec![3, 3, 3]
            })
            .await
            .unwrap();
        for (_node_id, headers) in headers_by_node.iter() {
            assert_eq!(headers.len(), 3);
            for header in headers.values() {
                assert!(verify_checkpointer_header_signature(&network, header.clone()).unwrap());
                assert_eq!(
                    header,
                    &CheckpointAttestation {
                        node_id: header.node_id,
                        epoch,
                        previous_state_root: [2; 32].into(),
                        next_state_root: [3; 32].into(),
                        serialized_state_digest: [1; 32],
                        // The signature is verified separately.
                        signature: header.signature,
                    }
                );
            }
        }

        // Check that the nodes have constructed and stored the aggregate checkpoint.
        let agg_header_by_node = wait_for_aggregate_checkpoint(&network, epoch, |header_by_node| {
            header_by_node.values().all(|header| header.is_some())
        })
        .await
        .unwrap();
        for (node_id, agg_header) in agg_header_by_node.iter() {
            // Verify the aggregate header signature.
            assert!(
                verify_aggregate_checkpointer_header(
                    &network,
                    agg_header.clone(),
                    *node_id,
                    headers_by_node.clone(),
                )
                .unwrap()
            );

            // Check that the aggregate header is correct.
            assert_eq!(
                agg_header,
                &AggregateCheckpoint {
                    epoch,
                    state_root: [3; 32].into(),
                    nodes: BitSet::from_iter(vec![0, 1, 2]),
                    // The signature is verified separately.
                    signature: agg_header.signature,
                }
            );
        }
    }

    // Shut down the network.
    network.shutdown().await;
}

#[tokio::test]
async fn test_no_supermajority_of_attestations() {
    let mut network = TestNetwork::builder()
        .with_committee_nodes::<TestFullNodeComponentsWithMockConsensus>(3)
        .await
        .build()
        .await
        .unwrap();
    let epoch = 1001;

    // Emit epoch changed notification to all nodes, with different state roots so that there is no
    // supermajority.
    // Here we have 2 nodes with the same next state root, and 1 node with a different next state
    // root. They all have the same epochs, serialized state digests, and previous state roots.
    network.node(0).emit_epoch_changed_notification(
        epoch,
        [1; 32],
        [2; 32].into(),
        [10; 32].into(),
    );
    network.node(1).emit_epoch_changed_notification(
        epoch,
        [1; 32],
        [2; 32].into(),
        [10; 32].into(),
    );
    network.node(2).emit_epoch_changed_notification(
        epoch,
        [1; 32],
        [2; 32].into(),
        [11; 32].into(),
    );

    // Check that the nodes have received and stored the checkpoint attestations.
    let headers_by_node = wait_for_checkpoint_attestations(&network, epoch, |headers_by_node| {
        headers_by_node
            .values()
            .map(|headers| headers.len())
            .collect::<Vec<_>>()
            == vec![3, 3, 3]
    })
    .await
    .unwrap();
    for headers in headers_by_node.values() {
        assert_eq!(headers.len(), 3);
        for header in headers.values() {
            assert!(verify_checkpointer_header_signature(&network, header.clone()).unwrap());
        }
    }

    // Check that the nodes have not stored an aggregate checkpoint, because there is no
    // supermajority.
    let result = wait_for_aggregate_checkpoint_with_timeout(
        &network,
        epoch,
        |header_by_node| header_by_node.values().all(|header| header.is_some()),
        Duration::from_secs(1),
    )
    .await;
    assert!(result.is_err());
    assert_eq!(result.unwrap_err(), PollUntilError::Timeout);

    // Shutdown the network.
    network.shutdown().await;
}

#[tokio::test]
async fn test_missing_epoch_change_notification_no_supermajority() {
    let mut network = TestNetwork::builder()
        .with_committee_nodes::<TestFullNodeComponentsWithMockConsensus>(3)
        .await
        .build()
        .await
        .unwrap();
    let epoch = 1001;

    // Emit epoch changed notification to all nodes, with different state roots so that there is no
    // supermajority.
    // Here we emit epoch changed notifications to two of the nodes, and not the third.
    network.node(0).emit_epoch_changed_notification(
        epoch,
        [1; 32],
        [2; 32].into(),
        [10; 32].into(),
    );
    network.node(1).emit_epoch_changed_notification(
        epoch,
        [1; 32],
        [2; 32].into(),
        [10; 32].into(),
    );

    // Check that the nodes have received and stored the checkpoint attestations.
    // Note that we only get 2 attestations per node, because one of the nodes did not receive an
    // epoch changed notification.
    let headers_by_node = wait_for_checkpoint_attestations(&network, epoch, |headers_by_node| {
        headers_by_node
            .values()
            .map(|headers| headers.len())
            .collect::<Vec<_>>()
            == vec![2, 2, 2]
    })
    .await
    .unwrap();
    for headers in headers_by_node.values() {
        assert_eq!(headers.len(), 2);
        for header in headers.values() {
            assert!(verify_checkpointer_header_signature(&network, header.clone()).unwrap());
        }
    }

    // Check that the nodes have not stored an aggregate checkpoint, because there is no
    // supermajority.
    let result = wait_for_aggregate_checkpoint_with_timeout(
        &network,
        epoch,
        |header_by_node| header_by_node.values().all(|header| header.is_some()),
        Duration::from_secs(1),
    )
    .await;
    assert!(result.is_err());
    assert_eq!(result.unwrap_err(), PollUntilError::Timeout);

    // Shutdown the network.
    network.shutdown().await;
}

#[tokio::test]
async fn test_missing_epoch_change_notification_still_supermajority() {
    let mut network = TestNetwork::builder()
        .with_committee_nodes::<TestFullNodeComponentsWithMockConsensus>(4)
        .await
        .build()
        .await
        .unwrap();
    let epoch = 1001;

    // Emit epoch changed notification to all nodes, with different state roots so that there is no
    // supermajority.
    // Here we emit epoch changed notifications to three of the nodes, and not the fourth, so that
    // there is still a supermajority.
    network.node(0).emit_epoch_changed_notification(
        epoch,
        [1; 32],
        [2; 32].into(),
        [10; 32].into(),
    );
    network.node(1).emit_epoch_changed_notification(
        epoch,
        [1; 32],
        [2; 32].into(),
        [10; 32].into(),
    );
    network.node(2).emit_epoch_changed_notification(
        epoch,
        [1; 32],
        [2; 32].into(),
        [10; 32].into(),
    );

    // Check that the nodes have received and stored the checkpoint attestations.
    // Note that we only get 2 attestations per node, because one of the nodes did not receive an
    // epoch changed notification.
    let headers_by_node = wait_for_checkpoint_attestations(&network, epoch, |headers_by_node| {
        headers_by_node
            .values()
            .map(|headers| headers.len())
            .collect::<Vec<_>>()
            == vec![3, 3, 3, 3]
    })
    .await
    .unwrap();
    for headers in headers_by_node.values() {
        assert_eq!(headers.len(), 3);
        for header in headers.values() {
            assert!(verify_checkpointer_header_signature(&network, header.clone()).unwrap());
        }
    }

    // Check that the nodes have constructed and stored the aggregate checkpoint.
    let agg_header_by_node = wait_for_aggregate_checkpoint(&network, epoch, |header_by_node| {
        header_by_node.values().all(|header| header.is_some())
    })
    .await
    .unwrap();
    for (node_id, agg_header) in agg_header_by_node.iter() {
        // Verify the aggregate header signature.
        assert!(
            verify_aggregate_checkpointer_header(
                &network,
                agg_header.clone(),
                *node_id,
                headers_by_node.clone(),
            )
            .unwrap()
        );

        // Check that the aggregate header is correct.
        assert_eq!(
            agg_header,
            &AggregateCheckpoint {
                epoch,
                state_root: [10; 32].into(),
                nodes: BitSet::from_iter(vec![0, 1, 2]),
                // The signature is verified separately.
                signature: agg_header.signature,
            }
        );
    }

    // Shutdown the network.
    network.shutdown().await;
}

#[tokio::test]
async fn test_aggregate_checkpoint_already_exists() {
    let mut network = TestNetwork::builder()
        .with_committee_nodes::<TestFullNodeComponentsWithMockConsensus>(1)
        .await
        .build()
        .await
        .unwrap();
    let epoch = 1001;

    // Emit epoch changed notifications.
    network.node(0).emit_epoch_changed_notification(
        epoch,
        [1; 32],
        [3; 32].into(),
        [10; 32].into(),
    );

    // Get the stored checkpoint attestations.
    let _headers_by_node =
        wait_for_checkpoint_attestations(&network, epoch, |headers| headers.len() == 1)
            .await
            .unwrap();

    // Get the stored aggregate checkpoint.
    let agg_header_by_node = wait_for_aggregate_checkpoint(&network, epoch, |header_by_node| {
        header_by_node.values().all(|header| header.is_some())
    })
    .await
    .unwrap();
    assert_eq!(agg_header_by_node.len(), 1);
    let expected_agg_header = AggregateCheckpoint {
        epoch,
        state_root: [10; 32].into(),
        nodes: BitSet::from_iter(vec![0]),
        signature: agg_header_by_node[&0].signature,
    };
    assert_eq!(agg_header_by_node[&0], expected_agg_header);

    // Emit the same epoch changed notification again, with a different state root so that
    // the resulting aggregate checkpoint is different.
    network
        .notify_epoch_changed(epoch, [4; 32].into(), [11; 32].into(), [2; 32])
        .await;

    // Check that the node has not stored a new aggregate checkpoint.
    let agg_header_by_node = wait_for_aggregate_checkpoint(&network, epoch, |header_by_node| {
        header_by_node.values().all(|header| header.is_some())
    })
    .await
    .unwrap();
    assert_eq!(agg_header_by_node.len(), 1);
    assert_eq!(agg_header_by_node[&0], expected_agg_header);

    // Shutdown the network.
    network.shutdown().await;
}

#[tokio::test]
async fn test_delayed_epoch_change_notification() {
    let mut network = TestNetwork::builder()
        .with_committee_nodes::<TestFullNodeComponentsWithMockConsensus>(3)
        .await
        .build()
        .await
        .unwrap();
    let epoch = 1001;

    // Emit epoch changed notification to 2 of 3 nodes.
    network.node(0).emit_epoch_changed_notification(
        epoch,
        [1; 32],
        [2; 32].into(),
        [10; 32].into(),
    );
    network.node(1).emit_epoch_changed_notification(
        epoch,
        [1; 32],
        [2; 32].into(),
        [10; 32].into(),
    );

    // Wait for 2 checkpoint attestations to be stored in all 3 nodes.
    let _headers_by_node = wait_for_checkpoint_attestations(&network, epoch, |headers_by_node| {
        headers_by_node.values().all(|headers| headers.len() == 2)
    })
    .await
    .unwrap();

    // Emit epoch changed notification to the third node.
    network.node(2).emit_epoch_changed_notification(
        epoch,
        [1; 32],
        [2; 32].into(),
        [10; 32].into(),
    );

    // Wait for the third node to receive the epoch changed notification, broadcast it's checkpoint
    // header, and for it to be stored in all the nodes.
    let headers_by_node = wait_for_checkpoint_attestations(&network, epoch, |headers_by_node| {
        headers_by_node.values().all(|headers| headers.len() == 3)
    })
    .await
    .unwrap();

    // Check that all the nodes have constructed and stored the aggregate checkpoint. In the
    // case of the third node, it's the responsibility of the epoch change listener itself because
    // nodes don't broadcast to themselves.
    let agg_header_by_node = wait_for_aggregate_checkpoint(&network, epoch, |header_by_node| {
        header_by_node.values().all(|header| header.is_some())
    })
    .await
    .unwrap();
    assert_eq!(agg_header_by_node.len(), 3);
    for (node_id, agg_header) in agg_header_by_node.iter() {
        assert!(
            verify_aggregate_checkpointer_header(
                &network,
                agg_header.clone(),
                *node_id,
                headers_by_node.clone(),
            )
            .unwrap()
        );
        assert_eq!(
            agg_header,
            &AggregateCheckpoint {
                epoch,
                state_root: [10; 32].into(),
                nodes: BitSet::from_iter(vec![0, 1, 2]),
                // The signature is verified separately.
                signature: agg_header.signature,
            }
        );
    }

    // Shutdown the network.
    network.shutdown().await;
}

#[tokio::test]
async fn test_multiple_different_epoch_change_notifications_simultaneously() {
    let mut network = TestNetwork::builder()
        .with_committee_nodes::<TestFullNodeComponentsWithMockConsensus>(3)
        .await
        .build()
        .await
        .unwrap();
    let epoch1 = 1001;
    let epoch2 = 1002;

    // Emit epoch changed notifications to all nodes for both epochs, interleaved for each node.
    network.node(0).emit_epoch_changed_notification(
        epoch1,
        [11; 32],
        [12; 32].into(),
        [110; 32].into(),
    );
    network.node(0).emit_epoch_changed_notification(
        epoch2,
        [21; 32],
        [22; 32].into(),
        [210; 32].into(),
    );
    network.node(1).emit_epoch_changed_notification(
        epoch1,
        [11; 32],
        [12; 32].into(),
        [110; 32].into(),
    );
    network.node(1).emit_epoch_changed_notification(
        epoch2,
        [21; 32],
        [22; 32].into(),
        [210; 32].into(),
    );
    network.node(2).emit_epoch_changed_notification(
        epoch1,
        [11; 32],
        [12; 32].into(),
        [110; 32].into(),
    );
    network.node(2).emit_epoch_changed_notification(
        epoch2,
        [21; 32],
        [22; 32].into(),
        [210; 32].into(),
    );

    // Check that the nodes have received and stored the checkpoint attestations for both epochs.
    let epoch1_headers_by_node =
        wait_for_checkpoint_attestations(&network, epoch1, |headers_by_node| {
            headers_by_node.values().all(|headers| headers.len() == 3)
        })
        .await
        .unwrap();
    for headers in epoch1_headers_by_node.values() {
        assert_eq!(headers.len(), 3);
        for header in headers.values() {
            assert!(verify_checkpointer_header_signature(&network, header.clone()).unwrap());
            assert_eq!(
                header,
                &CheckpointAttestation {
                    node_id: header.node_id,
                    epoch: epoch1,
                    previous_state_root: [12; 32].into(),
                    next_state_root: [110; 32].into(),
                    serialized_state_digest: [11; 32],
                    // The signature is verified separately.
                    signature: header.signature,
                }
            );
        }
    }
    let epoch2_headers_by_node =
        wait_for_checkpoint_attestations(&network, epoch2, |headers_by_node| {
            headers_by_node.values().all(|headers| headers.len() == 3)
        })
        .await
        .unwrap();
    for headers in epoch2_headers_by_node.values() {
        assert_eq!(headers.len(), 3);
        for header in headers.values() {
            assert!(verify_checkpointer_header_signature(&network, header.clone()).unwrap());
            assert_eq!(
                header,
                &CheckpointAttestation {
                    node_id: header.node_id,
                    epoch: epoch2,
                    previous_state_root: [22; 32].into(),
                    next_state_root: [210; 32].into(),
                    serialized_state_digest: [21; 32],
                    // The signature is verified separately.
                    signature: header.signature,
                }
            );
        }
    }

    // Check that the nodes have constructed and stored the aggregate checkpoints for both
    // epochs.
    let agg_header_by_node = wait_for_aggregate_checkpoint(&network, epoch1, |header_by_node| {
        header_by_node.values().all(|header| header.is_some())
    })
    .await
    .unwrap();
    for (node_id, agg_header) in agg_header_by_node.iter() {
        // Verify the aggregate header signature.
        assert!(
            verify_aggregate_checkpointer_header(
                &network,
                agg_header.clone(),
                *node_id,
                epoch1_headers_by_node.clone(),
            )
            .unwrap()
        );

        // Check that the aggregate header is correct.
        assert_eq!(
            agg_header,
            &AggregateCheckpoint {
                epoch: epoch1,
                state_root: [110; 32].into(),
                nodes: BitSet::from_iter(vec![0, 1, 2]),
                // The signature is verified separately.
                signature: agg_header.signature,
            }
        );
    }
    let agg_header_by_node = wait_for_aggregate_checkpoint(&network, epoch2, |header_by_node| {
        header_by_node.values().all(|header| header.is_some())
    })
    .await
    .unwrap();
    for (node_id, agg_header) in agg_header_by_node.iter() {
        // Verify the aggregate header signature.
        assert!(
            verify_aggregate_checkpointer_header(
                &network,
                agg_header.clone(),
                *node_id,
                epoch2_headers_by_node.clone(),
            )
            .unwrap()
        );

        // Check that the aggregate header is correct.
        assert_eq!(
            agg_header,
            &AggregateCheckpoint {
                epoch: epoch2,
                state_root: [210; 32].into(),
                nodes: BitSet::from_iter(vec![0, 1, 2]),
                // The signature is verified separately.
                signature: agg_header.signature,
            }
        );
    }

    // Shutdown the network.
    network.shutdown().await;
}

#[tokio::test]
async fn test_attestation_from_ineligible_node() {
    let mut network = TestNetwork::builder()
        .with_committee_nodes::<TestFullNodeComponentsWithMockConsensus>(2)
        .await
        .build()
        .await
        .unwrap();
    let epoch = 1001;

    // Send an invalid attestation to the node via the broadcaster.
    let other_node_keystore =
        EphemeralKeystore::<TestFullNodeComponentsWithMockConsensus>::default();
    let other_node_consensus_sk = other_node_keystore.get_bls_sk();
    let mut header = CheckpointAttestation {
        epoch,
        // From some some other non-existent node.
        node_id: 10,
        previous_state_root: [1; 32].into(),
        next_state_root: [2; 32].into(),
        serialized_state_digest: [3; 32],
        signature: ConsensusSignature::default(),
    };
    header.signature =
        other_node_consensus_sk.sign(DefaultSerdeBackend::serialize(&header).as_slice());
    broadcast_checkpoint_attestation_via_node(&network, 0, header)
        .await
        .unwrap();

    // Check that the node has not stored the invalid checkpoint attestation.
    let result = wait_for_checkpoint_attestations_with_timeout(
        &network,
        epoch,
        |headers_by_node| {
            headers_by_node
                .values()
                .map(|headers| headers.len())
                .collect::<Vec<_>>()
                == vec![1]
        },
        Duration::from_secs(1),
    )
    .await;
    assert!(result.is_err());
    assert_eq!(result.unwrap_err(), PollUntilError::Timeout);

    // Shutdown the network.
    network.shutdown().await;
}

#[tokio::test]
async fn test_attestation_with_invalid_signature() {
    let mut network = TestNetwork::builder()
        .with_committee_nodes::<TestFullNodeComponentsWithMockConsensus>(2)
        .await
        .build()
        .await
        .unwrap();
    let epoch = 1001;

    // Send an invalid attestation to the node via the broadcaster.
    let header = CheckpointAttestation {
        epoch,
        node_id: 1,
        previous_state_root: [1; 32].into(),
        next_state_root: [2; 32].into(),
        serialized_state_digest: [3; 32],
        signature: ConsensusSignature::default(),
    };
    broadcast_checkpoint_attestation_via_node(&network, 0, header)
        .await
        .unwrap();

    // Check that the node has not stored the invalid checkpoint attestation.
    let result = wait_for_checkpoint_attestations_with_timeout(
        &network,
        epoch,
        |headers_by_node| {
            headers_by_node
                .values()
                .map(|headers| headers.len())
                .collect::<Vec<_>>()
                == vec![1]
        },
        Duration::from_secs(1),
    )
    .await;
    assert!(result.is_err());
    assert_eq!(result.unwrap_err(), PollUntilError::Timeout);

    // Shutdown the network.
    network.shutdown().await;
}

#[tokio::test]
async fn test_attestations_with_inconsistent_state_roots_no_supermajority() {
    let mut network = TestNetwork::builder()
        .with_committee_nodes::<TestFullNodeComponentsWithMockConsensus>(3)
        .await
        .build()
        .await
        .unwrap();
    let epoch = 1001;

    // Send epoch change notifications to each node with different state roots.
    network
        .node(0)
        .emit_epoch_changed_notification(epoch, [1; 32], [2; 32].into(), [3; 32].into());
    network
        .node(1)
        .emit_epoch_changed_notification(epoch, [4; 32], [5; 32].into(), [6; 32].into());
    network
        .node(2)
        .emit_epoch_changed_notification(epoch, [7; 32], [8; 32].into(), [9; 32].into());

    // Check that the nodes have stored any checkpoint attestations.
    let _headers_by_node = wait_for_checkpoint_attestations(&network, epoch, |headers_by_node| {
        headers_by_node
            .values()
            .map(|headers| headers.len())
            .collect::<Vec<_>>()
            == vec![3, 3, 3]
    })
    .await
    .unwrap();

    // Check that the nodes have not stored the aggregate checkpoint, because they could not
    // reach agreement on state roots.
    let result = wait_for_aggregate_checkpoint_with_timeout(
        &network,
        epoch,
        |header_by_node| header_by_node.values().all(|header| header.is_some()),
        Duration::from_secs(1),
    )
    .await;
    assert!(result.is_err());
    assert_eq!(result.unwrap_err(), PollUntilError::Timeout);

    // Shutdown the network.
    network.shutdown().await;
}

#[tokio::test]
async fn test_attestations_with_inconsistent_state_roots_still_supermajority() {
    let mut network = TestNetwork::builder()
        .with_committee_nodes::<TestFullNodeComponentsWithMockConsensus>(4)
        .await
        .build()
        .await
        .unwrap();
    let epoch = 1001;

    // Send epoch change notifications to each node, with a different state root to just one, so
    // that we still have a supermajority.
    network
        .node(0)
        .emit_epoch_changed_notification(epoch, [1; 32], [2; 32].into(), [3; 32].into());
    network.node(1).emit_epoch_changed_notification(
        epoch,
        [10; 32],
        [11; 32].into(),
        [12; 32].into(),
    );
    network
        .node(2)
        .emit_epoch_changed_notification(epoch, [1; 32], [2; 32].into(), [3; 32].into());
    network
        .node(3)
        .emit_epoch_changed_notification(epoch, [1; 32], [2; 32].into(), [3; 32].into());

    // Check that the nodes have stored any checkpoint attestations.
    let headers_by_node = wait_for_checkpoint_attestations(&network, epoch, |headers_by_node| {
        headers_by_node
            .values()
            .map(|headers| headers.len())
            .collect::<Vec<_>>()
            >= vec![4, 4, 4, 4]
    })
    .await
    .unwrap();

    // Check that the nodes have not stored the aggregate checkpoint, because they could not
    // reach agreement on state roots.
    let agg_header_by_node = wait_for_aggregate_checkpoint(&network, epoch, |header_by_node| {
        header_by_node.values().all(|header| header.is_some())
    })
    .await
    .unwrap();
    assert_eq!(agg_header_by_node.len(), 4);

    for (node_id, agg_header) in agg_header_by_node.iter() {
        // Verify the aggregate header signature.
        assert!(
            verify_aggregate_checkpointer_header(
                &network,
                agg_header.clone(),
                *node_id,
                headers_by_node.clone(),
            )
            .unwrap()
        );

        // Check that the aggregate header is correct.
        assert_eq!(
            agg_header,
            &AggregateCheckpoint {
                epoch,
                state_root: [3; 32].into(),
                nodes: BitSet::from_iter(vec![0, 2, 3]),
                // The signature is verified separately.
                signature: agg_header.signature,
            }
        );
    }

    // Shutdown the network.
    network.shutdown().await;
}

#[tokio::test]
async fn test_multiple_different_epoch_change_notifications_for_same_epoch() {
    let mut network = TestNetwork::builder()
        .with_committee_nodes::<TestFullNodeComponentsWithMockConsensus>(2)
        .await
        .build()
        .await
        .unwrap();
    let epoch = 1001;

    // Send epoch change notifications to each node with different state roots.
    network
        .node(0)
        .emit_epoch_changed_notification(epoch, [1; 32], [2; 32].into(), [3; 32].into());
    network
        .node(1)
        .emit_epoch_changed_notification(epoch, [4; 32], [5; 32].into(), [6; 32].into());
    // Send another epoch change notification to the same node.
    network
        .node(1)
        .emit_epoch_changed_notification(epoch, [7; 32], [8; 32].into(), [9; 32].into());

    // Check that the nodes have stored any checkpoint attestations.
    let headers_by_node = wait_for_checkpoint_attestations(&network, epoch, |headers_by_node| {
        headers_by_node
            .values()
            .map(|headers| headers.len())
            .collect::<Vec<_>>()
            == vec![2, 2]
    })
    .await
    .unwrap();
    assert_eq!(headers_by_node.len(), 2);
    let expected_headers = vec![
        (
            0,
            CheckpointAttestation {
                epoch,
                node_id: 0,
                serialized_state_digest: [1; 32],
                previous_state_root: [2; 32].into(),
                next_state_root: [3; 32].into(),
                signature: headers_by_node[&0][&0].signature,
            },
        ),
        (
            1,
            CheckpointAttestation {
                epoch,
                node_id: 1,
                serialized_state_digest: [4; 32],
                previous_state_root: [5; 32].into(),
                next_state_root: [6; 32].into(),
                signature: headers_by_node[&0][&1].signature,
            },
        ),
    ]
    .into_iter()
    .collect::<HashMap<_, _>>();
    assert_eq!(headers_by_node[&0], expected_headers);
    assert_eq!(headers_by_node[&1], expected_headers);

    // Check that another header does not show up in the database.
    let result = wait_for_checkpoint_attestations_with_timeout(
        &network,
        epoch,
        |headers_by_node| {
            headers_by_node
                .values()
                .map(|headers| headers.len())
                .collect::<Vec<_>>()
                == vec![3, 3]
        },
        Duration::from_secs(1),
    )
    .await;
    assert!(result.is_err());
    assert_eq!(result.unwrap_err(), PollUntilError::Timeout);

    // Shutdown the network.
    network.shutdown().await;
}

#[tokio::test]
async fn test_multiple_different_attestations_from_same_node() {
    let mut network = TestNetwork::builder()
        .with_committee_nodes::<TestFullNodeComponentsWithMockConsensus>(2)
        .await
        .build()
        .await
        .unwrap();
    let epoch = 1001;

    // Broadcast the same attestation multiple times.
    let mut header = CheckpointAttestation {
        epoch,
        node_id: 0,
        previous_state_root: [1; 32].into(),
        next_state_root: [2; 32].into(),
        serialized_state_digest: [3; 32],
        signature: ConsensusSignature::default(),
    };
    header.signature = network
        .node(0)
        .get_consensus_secret_key()
        .sign(DefaultSerdeBackend::serialize(&header).as_slice());
    broadcast_checkpoint_attestation_via_node(&network, 0, header)
        .await
        .unwrap();

    // Check that the node has stored the attestation.
    let headers_by_node = wait_for_checkpoint_attestations(&network, epoch, |headers_by_node| {
        headers_by_node
            .iter()
            .map(|(node_id, headers)| (*node_id, headers.len()))
            .collect::<HashMap<_, _>>()
            == HashMap::from_iter(vec![(0, 0), (1, 1)])
    })
    .await
    .unwrap();
    assert_eq!(headers_by_node.len(), 2);

    // Broadcast another, different attestation from the same node.
    let mut header = CheckpointAttestation {
        epoch,
        node_id: 0,
        previous_state_root: [4; 32].into(),
        next_state_root: [5; 32].into(),
        serialized_state_digest: [6; 32],
        signature: ConsensusSignature::default(),
    };
    header.signature = network
        .node(0)
        .get_consensus_secret_key()
        .sign(DefaultSerdeBackend::serialize(&header).as_slice());
    broadcast_checkpoint_attestation_via_node(&network, 0, header)
        .await
        .unwrap();

    // Check that the node has not stored the second attestation.
    let result = wait_for_checkpoint_attestations_with_timeout(
        &network,
        epoch,
        |headers_by_node| {
            headers_by_node
                .iter()
                .map(|(node_id, headers)| (*node_id, headers.len()))
                .collect::<HashMap<_, _>>()
                == HashMap::from_iter(vec![(0, 0), (1, 2)])
        },
        Duration::from_secs(1),
    )
    .await;
    assert!(result.is_err());
    assert_eq!(result.unwrap_err(), PollUntilError::Timeout);

    // Shutdown the network.
    network.shutdown().await;
}

/// Send a checkpoint attestation to a specific node via their broadcaster pubsub.
pub async fn broadcast_checkpoint_attestation_via_node(
    network: &TestNetwork,
    node_id: NodeIndex,
    header: CheckpointAttestation,
) -> Result<()> {
    network
        .node(node_id)
        .downcast::<TestFullNodeComponentsWithMockConsensus>()
        .broadcast()
        .get_pubsub::<CheckpointBroadcastMessage>(Topic::Checkpoint)
        .send(
            &CheckpointBroadcastMessage::CheckpointAttestation(header),
            None,
        )
        .await
        .map_err(|e| anyhow::anyhow!(e))?;

    Ok(())
}

/// Wait for the checkpoint attestations to be received and stored by the checkpointer, and
/// matching the given condition function.
pub async fn wait_for_checkpoint_attestations<F>(
    network: &TestNetwork,
    epoch: Epoch,
    condition: F,
) -> Result<HashMap<NodeIndex, HashMap<NodeIndex, CheckpointAttestation>>, PollUntilError>
where
    F: Fn(&HashMap<NodeIndex, HashMap<NodeIndex, CheckpointAttestation>>) -> bool,
{
    const TIMEOUT: Duration = Duration::from_secs(10);

    wait_for_checkpoint_attestations_with_timeout(network, epoch, condition, TIMEOUT).await
}

pub async fn wait_for_checkpoint_attestations_with_timeout<F>(
    network: &TestNetwork,
    epoch: Epoch,
    condition: F,
    timeout: Duration,
) -> Result<HashMap<NodeIndex, HashMap<NodeIndex, CheckpointAttestation>>, PollUntilError>
where
    F: Fn(&HashMap<NodeIndex, HashMap<NodeIndex, CheckpointAttestation>>) -> bool,
{
    const DELAY: Duration = Duration::from_millis(100);

    poll_until(
        || async {
            let headers_by_node = network
                .nodes()
                .map(|node| {
                    let query = node
                        .downcast::<TestFullNodeComponentsWithMockConsensus>()
                        .checkpointer()
                        .query();
                    let headers = query.get_checkpoint_attestations(epoch);

                    (node.index(), headers)
                })
                .collect::<HashMap<_, _>>();

            condition(&headers_by_node)
                .then_some(headers_by_node)
                .ok_or(PollUntilError::ConditionNotSatisfied)
        },
        timeout,
        DELAY,
    )
    .await
}

/// Verify the signature of a checkpoint header.
pub fn verify_checkpointer_header_signature(
    network: &TestNetwork,
    header: CheckpointAttestation,
) -> Result<bool> {
    let header_node = network.node(header.node_id);
    Ok(header_node.get_consensus_public_key().verify(
        &header.signature,
        DefaultSerdeBackend::serialize(&CheckpointAttestation {
            signature: ConsensusSignature::default(),
            ..header.clone()
        })
        .as_slice(),
    )?)
}

/// Wait for the aggregate checkpoint to be received and stored by the checkpointer, and
/// matching the given condition function.
pub async fn wait_for_aggregate_checkpoint<F>(
    network: &TestNetwork,
    epoch: Epoch,
    condition: F,
) -> Result<HashMap<NodeIndex, AggregateCheckpoint>, PollUntilError>
where
    F: Fn(&HashMap<NodeIndex, Option<AggregateCheckpoint>>) -> bool,
{
    const TIMEOUT: Duration = Duration::from_secs(10);

    wait_for_aggregate_checkpoint_with_timeout(network, epoch, condition, TIMEOUT).await
}

pub async fn wait_for_aggregate_checkpoint_with_timeout<F>(
    network: &TestNetwork,
    epoch: Epoch,
    condition: F,
    timeout: Duration,
) -> Result<HashMap<NodeIndex, AggregateCheckpoint>, PollUntilError>
where
    F: Fn(&HashMap<NodeIndex, Option<AggregateCheckpoint>>) -> bool,
{
    const DELAY: Duration = Duration::from_millis(100);

    poll_until(
        || async {
            let header_by_node = network
                .nodes()
                .map(|node| {
                    let query = node
                        .downcast::<TestFullNodeComponentsWithMockConsensus>()
                        .checkpointer()
                        .query();
                    let header = query.get_aggregate_checkpoint(epoch);

                    (node.index(), header)
                })
                .collect::<HashMap<_, _>>();

            condition(&header_by_node)
                .then(|| {
                    header_by_node
                        .into_iter()
                        .map(|(node_id, header)| (node_id, header.unwrap()))
                        .collect::<HashMap<_, _>>()
                })
                .ok_or(PollUntilError::ConditionNotSatisfied)
        },
        timeout,
        DELAY,
    )
    .await
}

/// Verify the signature of an aggregate checkpoint.
pub fn verify_aggregate_checkpointer_header(
    network: &TestNetwork,
    agg_header: AggregateCheckpoint,
    node_id: NodeIndex,
    headers_by_node: HashMap<NodeIndex, HashMap<NodeIndex, CheckpointAttestation>>,
) -> Result<bool> {
    // Get public keys of all nodes in the aggregate header.
    let mut pks = agg_header
        .nodes
        .iter()
        .map(|node_id| {
            (
                node_id,
                network.node(node_id as u32).get_consensus_public_key(),
            )
        })
        .collect::<Vec<_>>();
    pks.sort_by_key(|(node_id, _)| *node_id);
    let pks = pks.into_iter().map(|(_, pk)| pk).collect::<Vec<_>>();

    // Get checkpoint attestation attestations for each node from the database.
    let mut attestations = headers_by_node[&node_id]
        .values()
        .filter(|h| agg_header.nodes.contains(h.node_id as usize))
        .collect::<Vec<_>>();
    attestations.sort_by_key(|header| header.node_id);
    let serialized_attestations = attestations
        .clone()
        .into_iter()
        .map(|header| {
            let header = CheckpointAttestation {
                signature: ConsensusSignature::default(),
                ..header.clone()
            };
            DefaultSerdeBackend::serialize(&header)
        })
        .collect::<Vec<_>>();

    // Verify the aggregate signature.
    let messages = serialized_attestations
        .iter()
        .map(|v| v.as_slice())
        .collect::<Vec<_>>();
    agg_header
        .signature
        .verify(&pks, &messages)
        .map_err(|e| anyhow::anyhow!(e))
}
