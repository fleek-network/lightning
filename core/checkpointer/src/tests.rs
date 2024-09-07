use std::collections::HashMap;
use std::time::Duration;

use atomo::{DefaultSerdeBackend, SerdeBackend};
use bit_set::BitSet;
use fleek_crypto::{ConsensusSignature, SecretKey};
use lightning_interfaces::types::{AggregateCheckpointHeader, CheckpointHeader};
use lightning_interfaces::KeystoreInterface;
use lightning_test_utils::keys::EphemeralKeystore;
use pretty_assertions::assert_eq;
use tempfile::tempdir;

use crate::test_utils::{TestNetworkBuilder, TestNodeBuilder, TestNodeComponents, WaitUntilError};

#[tokio::test]
async fn test_start_shutdown() {
    let temp_dir = tempdir().unwrap();
    let _node = TestNodeBuilder::new(temp_dir.path().to_path_buf())
        .build()
        .await
        .unwrap();
}

#[tokio::test]
async fn test_supermajority_over_epoch_changes() {
    let mut network = TestNetworkBuilder::new()
        .with_num_nodes(3)
        .build()
        .await
        .unwrap();

    for epoch in 0..10 {
        // Emit epoch changed notification to all nodes.
        network
            .notify_epoch_changed(epoch, [2; 32].into(), [3; 32].into(), [1; 32])
            .await;

        // Check that the nodes have received and stored the checkpoint headers.
        let headers_by_node = network
            .wait_for_checkpoint_headers(epoch, |headers_by_node| {
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
                assert!(
                    network
                        .verify_checkpointer_header_signature(header.clone())
                        .unwrap()
                );
                assert_eq!(
                    header,
                    &CheckpointHeader {
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

        // Check that the nodes have constructed and stored the aggregate checkpoint header.
        let agg_header_by_node = network
            .wait_for_aggregate_checkpoint_header(epoch, |header_by_node| {
                header_by_node.values().all(|header| header.is_some())
            })
            .await
            .unwrap();
        for (node_id, agg_header) in agg_header_by_node.iter() {
            // Verify the aggregate header signature.
            assert!(
                network
                    .verify_aggregate_checkpointer_header(
                        agg_header.clone(),
                        *node_id,
                        headers_by_node.clone(),
                    )
                    .unwrap()
            );

            // Check that the aggregate header is correct.
            assert_eq!(
                agg_header,
                &AggregateCheckpointHeader {
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
    let mut network = TestNetworkBuilder::new()
        .with_num_nodes(3)
        .build()
        .await
        .unwrap();
    let epoch = 1001;

    // Emit epoch changed notification to all nodes, with different state roots so that there is no
    // supermajority.
    // Here we have 2 nodes with the same next state root, and 1 node with a different next state
    // root. They all have the same epochs, serialized state digests, and previous state roots.
    network
        .notify_node_epoch_changed(0, epoch, [1; 32], [2; 32].into(), [10; 32].into())
        .await;
    network
        .notify_node_epoch_changed(1, epoch, [1; 32], [2; 32].into(), [10; 32].into())
        .await;
    network
        .notify_node_epoch_changed(2, epoch, [1; 32], [2; 32].into(), [11; 32].into())
        .await;

    // Check that the nodes have received and stored the checkpoint headers.
    let headers_by_node = network
        .wait_for_checkpoint_headers(epoch, |headers_by_node| {
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
            assert!(
                network
                    .verify_checkpointer_header_signature(header.clone())
                    .unwrap()
            );
        }
    }

    // Check that the nodes have not stored an aggregate checkpoint header, because there is no
    // supermajority.
    let result = network
        .wait_for_aggregate_checkpoint_header_with_timeout(
            epoch,
            |header_by_node| header_by_node.values().all(|header| header.is_some()),
            Duration::from_secs(1),
        )
        .await;
    assert!(result.is_err());
    assert_eq!(result.unwrap_err(), WaitUntilError::Timeout);

    // Shutdown the network.
    network.shutdown().await;
}

#[tokio::test]
async fn test_missing_epoch_change_notification_no_supermajority() {
    let mut network = TestNetworkBuilder::new()
        .with_num_nodes(3)
        .build()
        .await
        .unwrap();
    let epoch = 1001;

    // Emit epoch changed notification to all nodes, with different state roots so that there is no
    // supermajority.
    // Here we emit epoch changed notifications to two of the nodes, and not the third.
    network
        .notify_node_epoch_changed(0, epoch, [1; 32], [2; 32].into(), [10; 32].into())
        .await;
    network
        .notify_node_epoch_changed(1, epoch, [1; 32], [2; 32].into(), [10; 32].into())
        .await;

    // Check that the nodes have received and stored the checkpoint headers.
    // Note that we only get 2 headers per node, because one of the nodes did not receive an epoch
    // changed notification.
    let headers_by_node = network
        .wait_for_checkpoint_headers(epoch, |headers_by_node| {
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
            assert!(
                network
                    .verify_checkpointer_header_signature(header.clone())
                    .unwrap()
            );
        }
    }

    // Check that the nodes have not stored an aggregate checkpoint header, because there is no
    // supermajority.
    let result = network
        .wait_for_aggregate_checkpoint_header_with_timeout(
            epoch,
            |header_by_node| header_by_node.values().all(|header| header.is_some()),
            Duration::from_secs(1),
        )
        .await;
    assert!(result.is_err());
    assert_eq!(result.unwrap_err(), WaitUntilError::Timeout);

    // Shutdown the network.
    network.shutdown().await;
}

#[tokio::test]
async fn test_missing_epoch_change_notification_still_supermajority() {
    let mut network = TestNetworkBuilder::new()
        .with_num_nodes(4)
        .build()
        .await
        .unwrap();
    let epoch = 1001;

    // Emit epoch changed notification to all nodes, with different state roots so that there is no
    // supermajority.
    // Here we emit epoch changed notifications to three of the nodes, and not the fourth, so that
    // there is still a supermajority.
    network
        .notify_node_epoch_changed(0, epoch, [1; 32], [2; 32].into(), [10; 32].into())
        .await;
    network
        .notify_node_epoch_changed(1, epoch, [1; 32], [2; 32].into(), [10; 32].into())
        .await;
    network
        .notify_node_epoch_changed(2, epoch, [1; 32], [2; 32].into(), [10; 32].into())
        .await;

    // Check that the nodes have received and stored the checkpoint headers.
    // Note that we only get 2 headers per node, because one of the nodes did not receive an epoch
    // changed notification.
    let headers_by_node = network
        .wait_for_checkpoint_headers(epoch, |headers_by_node| {
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
            assert!(
                network
                    .verify_checkpointer_header_signature(header.clone())
                    .unwrap()
            );
        }
    }

    // Check that the nodes have constructed and stored the aggregate checkpoint header.
    let agg_header_by_node = network
        .wait_for_aggregate_checkpoint_header(epoch, |header_by_node| {
            header_by_node.values().all(|header| header.is_some())
        })
        .await
        .unwrap();
    for (node_id, agg_header) in agg_header_by_node.iter() {
        // Verify the aggregate header signature.
        assert!(
            network
                .verify_aggregate_checkpointer_header(
                    agg_header.clone(),
                    *node_id,
                    headers_by_node.clone(),
                )
                .unwrap()
        );

        // Check that the aggregate header is correct.
        assert_eq!(
            agg_header,
            &AggregateCheckpointHeader {
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
async fn test_aggregate_checkpoint_header_already_exists() {
    let mut network = TestNetworkBuilder::new()
        .with_num_nodes(1)
        .build()
        .await
        .unwrap();
    let epoch = 1001;

    // Emit epoch changed notifications.
    network
        .notify_epoch_changed(epoch, [3; 32].into(), [10; 32].into(), [1; 32])
        .await;

    // Get the stored checkpoint headers.
    let _headers_by_node = network
        .wait_for_checkpoint_headers(epoch, |headers| headers.len() == 1)
        .await
        .unwrap();

    // Get the stored aggregate checkpoint header.
    let agg_header_by_node = network
        .wait_for_aggregate_checkpoint_header(epoch, |header_by_node| {
            header_by_node.values().all(|header| header.is_some())
        })
        .await
        .unwrap();
    assert_eq!(agg_header_by_node.len(), 1);
    let expected_agg_header = AggregateCheckpointHeader {
        epoch,
        state_root: [10; 32].into(),
        nodes: BitSet::from_iter(vec![0]),
        signature: agg_header_by_node[&0].signature,
    };
    assert_eq!(agg_header_by_node[&0], expected_agg_header);

    // Emit the same epoch changed notification again, with a different state root so that
    // the resulting aggregate checkpoint header is different.
    network
        .notify_epoch_changed(epoch, [4; 32].into(), [11; 32].into(), [2; 32])
        .await;

    // Check that the node has not stored a new aggregate checkpoint header.
    let agg_header_by_node = network
        .wait_for_aggregate_checkpoint_header(epoch, |header_by_node| {
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
    let mut network = TestNetworkBuilder::new()
        .with_num_nodes(3)
        .build()
        .await
        .unwrap();
    let epoch = 1001;

    // Emit epoch changed notification to 2 of 3 nodes.
    network
        .notify_node_epoch_changed(0, epoch, [1; 32], [2; 32].into(), [10; 32].into())
        .await;
    network
        .notify_node_epoch_changed(1, epoch, [1; 32], [2; 32].into(), [10; 32].into())
        .await;

    // Wait for 2 checkpoint headers to be stored in all 3 nodes.
    let _headers_by_node = network
        .wait_for_checkpoint_headers(epoch, |headers_by_node| {
            headers_by_node.values().all(|headers| headers.len() == 2)
        })
        .await
        .unwrap();

    // Emit epoch changed notification to the third node.
    network
        .notify_node_epoch_changed(2, epoch, [1; 32], [2; 32].into(), [10; 32].into())
        .await;

    // Wait for the third node to receive the epoch changed notification, broadcast it's checkpoint
    // header, and for it to be stored in all the nodes.
    let headers_by_node = network
        .wait_for_checkpoint_headers(epoch, |headers_by_node| {
            headers_by_node.values().all(|headers| headers.len() == 3)
        })
        .await
        .unwrap();

    // Check that all the nodes have constructed and stored the aggregate checkpoint header. In the
    // case of the third node, it's the responsibility of the epoch change listener itself because
    // nodes don't broadcast to themselves.
    let agg_header_by_node = network
        .wait_for_aggregate_checkpoint_header(epoch, |header_by_node| {
            header_by_node.values().all(|header| header.is_some())
        })
        .await
        .unwrap();
    assert_eq!(agg_header_by_node.len(), 3);
    for (node_id, agg_header) in agg_header_by_node.iter() {
        assert!(
            network
                .verify_aggregate_checkpointer_header(
                    agg_header.clone(),
                    *node_id,
                    headers_by_node.clone(),
                )
                .unwrap()
        );
        assert_eq!(
            agg_header,
            &AggregateCheckpointHeader {
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
    let mut network = TestNetworkBuilder::new()
        .with_num_nodes(3)
        .build()
        .await
        .unwrap();
    let epoch1 = 1001;
    let epoch2 = 1002;

    // Emit epoch changed notifications to all nodes for both epochs, interleaved for each node.
    network
        .notify_node_epoch_changed(0, epoch1, [11; 32], [12; 32].into(), [110; 32].into())
        .await;
    network
        .notify_node_epoch_changed(0, epoch2, [21; 32], [22; 32].into(), [210; 32].into())
        .await;
    network
        .notify_node_epoch_changed(1, epoch1, [11; 32], [12; 32].into(), [110; 32].into())
        .await;
    network
        .notify_node_epoch_changed(1, epoch2, [21; 32], [22; 32].into(), [210; 32].into())
        .await;
    network
        .notify_node_epoch_changed(2, epoch1, [11; 32], [12; 32].into(), [110; 32].into())
        .await;
    network
        .notify_node_epoch_changed(2, epoch2, [21; 32], [22; 32].into(), [210; 32].into())
        .await;

    // Check that the nodes have received and stored the checkpoint headers for both epochs.
    let epoch1_headers_by_node = network
        .wait_for_checkpoint_headers(epoch1, |headers_by_node| {
            headers_by_node.values().all(|headers| headers.len() == 3)
        })
        .await
        .unwrap();
    for headers in epoch1_headers_by_node.values() {
        assert_eq!(headers.len(), 3);
        for header in headers.values() {
            assert!(
                network
                    .verify_checkpointer_header_signature(header.clone())
                    .unwrap()
            );
            assert_eq!(
                header,
                &CheckpointHeader {
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
    let epoch2_headers_by_node = network
        .wait_for_checkpoint_headers(epoch2, |headers_by_node| {
            headers_by_node.values().all(|headers| headers.len() == 3)
        })
        .await
        .unwrap();
    for headers in epoch2_headers_by_node.values() {
        assert_eq!(headers.len(), 3);
        for header in headers.values() {
            assert!(
                network
                    .verify_checkpointer_header_signature(header.clone())
                    .unwrap()
            );
            assert_eq!(
                header,
                &CheckpointHeader {
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

    // Check that the nodes have constructed and stored the aggregate checkpoint headers for both
    // epochs.
    let agg_header_by_node = network
        .wait_for_aggregate_checkpoint_header(epoch1, |header_by_node| {
            header_by_node.values().all(|header| header.is_some())
        })
        .await
        .unwrap();
    for (node_id, agg_header) in agg_header_by_node.iter() {
        // Verify the aggregate header signature.
        assert!(
            network
                .verify_aggregate_checkpointer_header(
                    agg_header.clone(),
                    *node_id,
                    epoch1_headers_by_node.clone(),
                )
                .unwrap()
        );

        // Check that the aggregate header is correct.
        assert_eq!(
            agg_header,
            &AggregateCheckpointHeader {
                epoch: epoch1,
                state_root: [110; 32].into(),
                nodes: BitSet::from_iter(vec![0, 1, 2]),
                // The signature is verified separately.
                signature: agg_header.signature,
            }
        );
    }
    let agg_header_by_node = network
        .wait_for_aggregate_checkpoint_header(epoch2, |header_by_node| {
            header_by_node.values().all(|header| header.is_some())
        })
        .await
        .unwrap();
    for (node_id, agg_header) in agg_header_by_node.iter() {
        // Verify the aggregate header signature.
        assert!(
            network
                .verify_aggregate_checkpointer_header(
                    agg_header.clone(),
                    *node_id,
                    epoch2_headers_by_node.clone(),
                )
                .unwrap()
        );

        // Check that the aggregate header is correct.
        assert_eq!(
            agg_header,
            &AggregateCheckpointHeader {
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
    let mut network = TestNetworkBuilder::new()
        .with_num_nodes(2)
        .build()
        .await
        .unwrap();
    let epoch = 1001;

    // Send an invalid attestation to the node via the broadcaster.
    let other_node_keystore = EphemeralKeystore::<TestNodeComponents>::default();
    let other_node_consensus_sk = other_node_keystore.get_bls_sk();
    let mut header = CheckpointHeader {
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
    network
        .broadcast_checkpoint_header_via_node(0, header)
        .await
        .unwrap();

    // Check that the node has not stored the invalid checkpoint header.
    let result = network
        .wait_for_checkpoint_headers_with_timeout(
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
    assert_eq!(result.unwrap_err(), WaitUntilError::Timeout);

    // Shutdown the network.
    network.shutdown().await;
}

#[tokio::test]
async fn test_attestation_with_invalid_signature() {
    let mut network = TestNetworkBuilder::new()
        .with_num_nodes(2)
        .build()
        .await
        .unwrap();
    let epoch = 1001;

    // Send an invalid attestation to the node via the broadcaster.
    let header = CheckpointHeader {
        epoch,
        node_id: 1,
        previous_state_root: [1; 32].into(),
        next_state_root: [2; 32].into(),
        serialized_state_digest: [3; 32],
        signature: ConsensusSignature::default(),
    };
    network
        .broadcast_checkpoint_header_via_node(0, header)
        .await
        .unwrap();

    // Check that the node has not stored the invalid checkpoint header.
    let result = network
        .wait_for_checkpoint_headers_with_timeout(
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
    assert_eq!(result.unwrap_err(), WaitUntilError::Timeout);

    // Shutdown the network.
    network.shutdown().await;
}

#[tokio::test]
async fn test_attestations_with_inconsistent_state_roots_no_supermajority() {
    let mut network = TestNetworkBuilder::new()
        .with_num_nodes(3)
        .build()
        .await
        .unwrap();
    let epoch = 1001;

    // Send epoch change notifications to each node with different state roots.
    network
        .notify_node_epoch_changed(0, epoch, [1; 32], [2; 32].into(), [3; 32].into())
        .await;
    network
        .notify_node_epoch_changed(1, epoch, [4; 32], [5; 32].into(), [6; 32].into())
        .await;
    network
        .notify_node_epoch_changed(2, epoch, [7; 32], [8; 32].into(), [9; 32].into())
        .await;

    // Check that the nodes have stored any checkpoint headers.
    let _headers_by_node = network
        .wait_for_checkpoint_headers(epoch, |headers_by_node| {
            headers_by_node
                .values()
                .map(|headers| headers.len())
                .collect::<Vec<_>>()
                == vec![3, 3, 3]
        })
        .await
        .unwrap();

    // Check that the nodes have not stored the aggregate checkpoint header, because they could not
    // reach agreement on state roots.
    let result = network
        .wait_for_aggregate_checkpoint_header_with_timeout(
            epoch,
            |header_by_node| header_by_node.values().all(|header| header.is_some()),
            Duration::from_secs(1),
        )
        .await;
    assert!(result.is_err());
    assert_eq!(result.unwrap_err(), WaitUntilError::Timeout);

    // Shutdown the network.
    network.shutdown().await;
}

#[tokio::test]
async fn test_attestations_with_inconsistent_state_roots_still_supermajority() {
    let mut network = TestNetworkBuilder::new()
        .with_num_nodes(4)
        .build()
        .await
        .unwrap();
    let epoch = 1001;

    // Send epoch change notifications to each node, with a different state root to just one, so
    // that we still have a supermajority.
    network
        .notify_node_epoch_changed(0, epoch, [1; 32], [2; 32].into(), [3; 32].into())
        .await;
    network
        .notify_node_epoch_changed(1, epoch, [10; 32], [11; 32].into(), [12; 32].into())
        .await;
    network
        .notify_node_epoch_changed(2, epoch, [1; 32], [2; 32].into(), [3; 32].into())
        .await;
    network
        .notify_node_epoch_changed(3, epoch, [1; 32], [2; 32].into(), [3; 32].into())
        .await;

    // Check that the nodes have stored any checkpoint headers.
    let headers_by_node = network
        .wait_for_checkpoint_headers(epoch, |headers_by_node| {
            headers_by_node
                .values()
                .map(|headers| headers.len())
                .collect::<Vec<_>>()
                >= vec![4, 4, 4, 4]
        })
        .await
        .unwrap();

    // Check that the nodes have not stored the aggregate checkpoint header, because they could not
    // reach agreement on state roots.
    let agg_header_by_node = network
        .wait_for_aggregate_checkpoint_header(epoch, |header_by_node| {
            header_by_node.values().all(|header| header.is_some())
        })
        .await
        .unwrap();
    assert_eq!(agg_header_by_node.len(), 4);

    for (node_id, agg_header) in agg_header_by_node.iter() {
        // Verify the aggregate header signature.
        assert!(
            network
                .verify_aggregate_checkpointer_header(
                    agg_header.clone(),
                    *node_id,
                    headers_by_node.clone(),
                )
                .unwrap()
        );

        // Check that the aggregate header is correct.
        assert_eq!(
            agg_header,
            &AggregateCheckpointHeader {
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
    let mut network = TestNetworkBuilder::new()
        .with_num_nodes(2)
        .build()
        .await
        .unwrap();
    let epoch = 1001;

    // Send epoch change notifications to each node with different state roots.
    network
        .notify_node_epoch_changed(0, epoch, [1; 32], [2; 32].into(), [3; 32].into())
        .await;
    network
        .notify_node_epoch_changed(1, epoch, [4; 32], [5; 32].into(), [6; 32].into())
        .await;
    // Send another epoch change notification to the same node.
    network
        .notify_node_epoch_changed(1, epoch, [7; 32], [8; 32].into(), [9; 32].into())
        .await;

    // Check that the nodes have stored any checkpoint headers.
    let headers_by_node = network
        .wait_for_checkpoint_headers(epoch, |headers_by_node| {
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
            CheckpointHeader {
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
            CheckpointHeader {
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
    let result = network
        .wait_for_checkpoint_headers_with_timeout(
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
    assert_eq!(result.unwrap_err(), WaitUntilError::Timeout);

    // Shutdown the network.
    network.shutdown().await;
}

#[tokio::test]
async fn test_multiple_different_attestations_from_same_node() {
    let mut network = TestNetworkBuilder::new()
        .with_num_nodes(2)
        .build()
        .await
        .unwrap();
    let epoch = 1001;

    // Broadcast the same attestation multiple times.
    let mut header = CheckpointHeader {
        epoch,
        node_id: 0,
        previous_state_root: [1; 32].into(),
        next_state_root: [2; 32].into(),
        serialized_state_digest: [3; 32],
        signature: ConsensusSignature::default(),
    };
    header.signature = network
        .node(0)
        .unwrap()
        .get_consensus_secret_key()
        .sign(DefaultSerdeBackend::serialize(&header).as_slice());
    network
        .broadcast_checkpoint_header_via_node(0, header)
        .await
        .unwrap();

    // Check that the node has stored the attestation.
    let headers_by_node = network
        .wait_for_checkpoint_headers(epoch, |headers_by_node| {
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
    let mut header = CheckpointHeader {
        epoch,
        node_id: 0,
        previous_state_root: [4; 32].into(),
        next_state_root: [5; 32].into(),
        serialized_state_digest: [6; 32],
        signature: ConsensusSignature::default(),
    };
    header.signature = network
        .node(0)
        .unwrap()
        .get_consensus_secret_key()
        .sign(DefaultSerdeBackend::serialize(&header).as_slice());
    network
        .broadcast_checkpoint_header_via_node(0, header)
        .await
        .unwrap();

    // Check that the node has not stored the second attestation.
    let result = network
        .wait_for_checkpoint_headers_with_timeout(
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
    assert_eq!(result.unwrap_err(), WaitUntilError::Timeout);

    // Shutdown the network.
    network.shutdown().await;
}
