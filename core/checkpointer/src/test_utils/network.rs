use std::collections::{HashMap, HashSet};
use std::time::Duration;

use anyhow::Result;
use atomo::{DefaultSerdeBackend, SerdeBackend};
use fleek_crypto::{ConsensusSignature, PublicKey};
use futures::future::join_all;
use lightning_interfaces::types::{AggregateCheckpointHeader, CheckpointHeader, Epoch, NodeIndex};
use lightning_interfaces::{CheckpointerInterface, CheckpointerQueryInterface, KeystoreInterface};
use tempfile::TempDir;

use super::{wait_until, TestNode};

/// A network of test nodes.
///
/// This encapsulates the management of nodes and provides methods to interact with them.
pub struct TestNetwork {
    _temp_dir: TempDir,
    pub node_by_id: HashMap<NodeIndex, TestNode>,
}

impl TestNetwork {
    pub async fn new(temp_dir: TempDir, nodes: Vec<TestNode>) -> Result<Self> {
        Ok(Self {
            _temp_dir: temp_dir,

            // We assume that at this point the genesis has been applied, otherwise this will panic.
            node_by_id: nodes
                .into_iter()
                .map(|node| (node.get_id().expect("node id not found"), node))
                .collect::<HashMap<_, _>>(),
        })
    }

    pub fn nodes(&self) -> impl Iterator<Item = &TestNode> {
        self.node_by_id.values()
    }

    pub fn node(&self, node_id: NodeIndex) -> Option<&TestNode> {
        self.node_by_id.get(&node_id)
    }

    pub async fn shutdown(&mut self) {
        join_all(self.node_by_id.values_mut().map(|node| node.shutdown())).await;
    }

    /// Wait for the checkpoint headers to be received and stored by the checkpointer, and matching
    /// the given condition function.
    pub fn wait_for_checkpoint_headers<F>(
        &self,
        epoch: Epoch,
        condition: F,
    ) -> HashMap<NodeIndex, HashSet<CheckpointHeader>>
    where
        F: Fn(&HashMap<NodeIndex, HashSet<CheckpointHeader>>) -> bool,
    {
        wait_until(
            || {
                let headers_by_node = self
                    .node_by_id
                    .iter()
                    .map(|(node_id, node)| {
                        let query = node.checkpointer.query();
                        let headers = query.get_checkpoint_headers(epoch);

                        (*node_id, headers)
                    })
                    .collect::<HashMap<_, _>>();

                if !condition(&headers_by_node) {
                    return None;
                }

                Some(headers_by_node)
            },
            Duration::from_secs(2),
            Duration::from_millis(100),
        )
        .unwrap()
    }

    /// Verify the signature of a checkpoint header.
    pub fn verify_checkpointer_header_signature(&self, header: CheckpointHeader) -> Result<bool> {
        let header_node = self.node(header.node_id).unwrap();
        Ok(header_node.keystore.get_bls_pk().verify(
            &header.signature,
            DefaultSerdeBackend::serialize(&CheckpointHeader {
                signature: ConsensusSignature::default(),
                ..header.clone()
            })
            .as_slice(),
        ))
    }

    /// Wait for the aggregate checkpoint header to be received and stored by the checkpointer, and
    /// matching the given condition function.
    pub fn wait_for_aggregate_checkpoint_header<F>(
        &self,
        epoch: Epoch,
        condition: F,
    ) -> HashMap<NodeIndex, AggregateCheckpointHeader>
    where
        F: Fn(&HashMap<NodeIndex, Option<AggregateCheckpointHeader>>) -> bool,
    {
        wait_until(
            || {
                let header_by_node = self
                    .node_by_id
                    .iter()
                    .map(|(node_id, node)| {
                        let query = node.checkpointer.query();
                        let header = query.get_aggregate_checkpoint_header(epoch);

                        (*node_id, header)
                    })
                    .collect::<HashMap<_, _>>();

                if !condition(&header_by_node) {
                    return None;
                }

                Some(
                    header_by_node
                        .into_iter()
                        .map(|(node_id, header)| (node_id, header.unwrap()))
                        .collect::<HashMap<_, _>>(),
                )
            },
            Duration::from_secs(2),
            Duration::from_millis(100),
        )
        .unwrap()
    }

    /// Verify the signature of an aggregate checkpoint header.
    pub fn verify_aggregate_checkpointer_header(
        &self,
        agg_header: AggregateCheckpointHeader,
        node_id: NodeIndex,
        headers_by_node: HashMap<NodeIndex, HashSet<CheckpointHeader>>,
    ) -> Result<bool> {
        // Get public keys of all nodes in the aggregate header.
        let mut pks = agg_header
            .nodes
            .iter()
            .map(|node_id| {
                (
                    node_id,
                    self.node(node_id as u32).unwrap().keystore.get_bls_pk(),
                )
            })
            .collect::<Vec<_>>();
        pks.sort_by_key(|(node_id, _)| *node_id);
        let pks = pks.into_iter().map(|(_, pk)| pk).collect::<Vec<_>>();

        // Get checkpoint header attestations for each node from the database.
        let mut attestations = headers_by_node[&node_id]
            .iter()
            .filter(|h| agg_header.nodes.contains(h.node_id as usize))
            .collect::<Vec<_>>();
        attestations.sort_by_key(|header| header.node_id);
        let serialized_attestations = attestations
            .clone()
            .into_iter()
            .map(|header| {
                let header = CheckpointHeader {
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
}
