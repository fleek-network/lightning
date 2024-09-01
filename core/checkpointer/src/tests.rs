use anyhow::Result;
use bit_set::BitSet;
use lightning_interfaces::types::{AggregateCheckpointHeader, CheckpointHeader};
use lightning_interfaces::{Emitter, NotifierInterface};
use pretty_assertions::assert_eq;
use tempfile::tempdir;

use crate::test_utils::{TestNetworkBuilder, TestNodeBuilder};

#[tokio::test]
async fn test_checkpointer_start_shutdown() -> Result<()> {
    let temp_dir = tempdir()?;
    let _node = TestNodeBuilder::new(temp_dir.path().to_path_buf())
        .build()
        .await?;

    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn test_checkpointer_over_single_epoch_change() -> Result<()> {
    // TODO(snormore): Remove this logger setup.
    let _ = tracing_subscriber::fmt::try_init();

    let mut network = TestNetworkBuilder::new().with_num_nodes(3).build().await?;

    // Emit epoch changed notification to all nodes.
    for node in network.nodes() {
        node.notifier
            .get_emitter()
            // TODO(snormore): Use more realistic values here.
            .epoch_changed(0, [0; 32], [0; 32], [0; 32]);
    }

    // Check that the nodes have received and stored the checkpoint headers.
    let headers_by_node = network.wait_for_checkpoint_headers(0, |headers_by_node| {
        headers_by_node
            .values()
            .map(|headers| headers.len())
            .collect::<Vec<_>>()
            == vec![3, 3, 3]
    });
    for (_node_id, headers) in headers_by_node.iter() {
        assert_eq!(headers.len(), 3);
        for header in headers.iter() {
            assert!(network.verify_checkpointer_header_signature(header.clone())?);
            assert_eq!(
                header,
                &CheckpointHeader {
                    node_id: header.node_id,
                    epoch: 0,
                    previous_state_root: [0; 32],
                    next_state_root: [0; 32],
                    serialized_state_digest: [0; 32],
                    // The signature is verified separately.
                    signature: header.signature,
                }
            );
        }
    }

    // Check that the nodes have constructed and stored the aggregate checkpoint header.
    let agg_header_by_node = network.wait_for_aggregate_checkpoint_header(0, |header_by_node| {
        header_by_node.values().all(|header| header.is_some())
    });
    for (node_id, agg_header) in agg_header_by_node.iter() {
        // Verify the aggregate header signature.
        assert!(network.verify_aggregate_checkpointer_header(
            agg_header.clone(),
            *node_id,
            headers_by_node.clone(),
        )?);

        // Check that the aggregate header is correct.
        assert_eq!(
            agg_header,
            &AggregateCheckpointHeader {
                epoch: 0,
                previous_state_root: [0; 32],
                next_state_root: [0; 32],
                nodes: BitSet::from_iter(vec![0, 1, 2]),
                // The signature is verified separately.
                signature: agg_header.signature,
            }
        );
    }

    network.shutdown().await;
    Ok(())
}

// #[tokio::test]
// async fn test_checkpointer_over_many_epoch_changes() -> Result<()> {
//     // TODO(snormore): Implement this test.
//     Ok(())
// }

// #[tokio::test]
// async fn test_checkpointer_no_supermajority_of_attestations() -> Result<()> {
//     // TODO(snormore): Implement this test.
//     Ok(())
// }

// #[tokio::test]
// async fn test_checkpointer_fake_and_corrupt_attestation() -> Result<()> {
//     // TODO(snormore): Implement this test.
//     Ok(())
// }

// #[tokio::test]
// async fn test_checkpointer_duplicate_attestations() -> Result<()> {
//     // TODO(snormore): Implement this test.
//     Ok(())
// }

// #[tokio::test]
// async fn test_checkpointer_too_few_attestations() -> Result<()> {
//     // TODO(snormore): Implement this test.
//     Ok(())
// }

// #[tokio::test]
// async fn test_checkpointer_missing_attestations() -> Result<()> {
//     // TODO(snormore): Implement this test.
//     Ok(())
// }
