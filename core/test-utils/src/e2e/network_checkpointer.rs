use std::collections::HashMap;
use std::time::Duration;

use anyhow::Result;
use atomo::{DefaultSerdeBackend, SerdeBackend};
use fleek_crypto::{ConsensusSignature, PublicKey};
use lightning_checkpointer::CheckpointBroadcastMessage;
use lightning_interfaces::prelude::*;
use lightning_interfaces::types::{
    AggregateCheckpointHeader,
    CheckpointHeader,
    Epoch,
    NodeIndex,
    Topic,
};

use super::{TestNetwork, WaitUntilError};
use crate::e2e::wait_until;

impl TestNetwork {
    /// Send a checkpoint attestation to a specific node via their broadcaster pubsub.
    pub async fn broadcast_checkpoint_header_via_node(
        &self,
        node_id: NodeIndex,
        header: CheckpointHeader,
    ) -> Result<()> {
        self.node(node_id)
            .broadcast
            .get_pubsub::<CheckpointBroadcastMessage>(Topic::Checkpoint)
            .send(&CheckpointBroadcastMessage::CheckpointHeader(header), None)
            .await
            .map_err(|e| anyhow::anyhow!(e))?;

        Ok(())
    }

    /// Wait for the checkpoint headers to be received and stored by the checkpointer, and matching
    /// the given condition function.
    pub async fn wait_for_checkpoint_headers<F>(
        &self,
        epoch: Epoch,
        condition: F,
    ) -> Result<HashMap<NodeIndex, HashMap<NodeIndex, CheckpointHeader>>, WaitUntilError>
    where
        F: Fn(&HashMap<NodeIndex, HashMap<NodeIndex, CheckpointHeader>>) -> bool,
    {
        const TIMEOUT: Duration = Duration::from_secs(10);

        self.wait_for_checkpoint_headers_with_timeout(epoch, condition, TIMEOUT)
            .await
    }

    pub async fn wait_for_checkpoint_headers_with_timeout<F>(
        &self,
        epoch: Epoch,
        condition: F,
        timeout: Duration,
    ) -> Result<HashMap<NodeIndex, HashMap<NodeIndex, CheckpointHeader>>, WaitUntilError>
    where
        F: Fn(&HashMap<NodeIndex, HashMap<NodeIndex, CheckpointHeader>>) -> bool,
    {
        const DELAY: Duration = Duration::from_millis(100);

        wait_until(
            || async {
                let headers_by_node = self
                    .node_by_id
                    .iter()
                    .map(|(node_id, node)| {
                        let query = node.checkpointer.query();
                        let headers = query.get_checkpoint_headers(epoch);

                        (*node_id, headers)
                    })
                    .collect::<HashMap<_, _>>();

                if condition(&headers_by_node) {
                    Some(headers_by_node)
                } else {
                    None
                }
            },
            timeout,
            DELAY,
        )
        .await
    }

    /// Verify the signature of a checkpoint header.
    pub fn verify_checkpointer_header_signature(&self, header: CheckpointHeader) -> Result<bool> {
        let header_node = self.node(header.node_id);
        Ok(header_node.keystore.get_bls_pk().verify(
            &header.signature,
            DefaultSerdeBackend::serialize(&CheckpointHeader {
                signature: ConsensusSignature::default(),
                ..header.clone()
            })
            .as_slice(),
        )?)
    }

    /// Wait for the aggregate checkpoint header to be received and stored by the checkpointer, and
    /// matching the given condition function.
    pub async fn wait_for_aggregate_checkpoint_header<F>(
        &self,
        epoch: Epoch,
        condition: F,
    ) -> Result<HashMap<NodeIndex, AggregateCheckpointHeader>, WaitUntilError>
    where
        F: Fn(&HashMap<NodeIndex, Option<AggregateCheckpointHeader>>) -> bool,
    {
        const TIMEOUT: Duration = Duration::from_secs(10);

        self.wait_for_aggregate_checkpoint_header_with_timeout(epoch, condition, TIMEOUT)
            .await
    }

    pub async fn wait_for_aggregate_checkpoint_header_with_timeout<F>(
        &self,
        epoch: Epoch,
        condition: F,
        timeout: Duration,
    ) -> Result<HashMap<NodeIndex, AggregateCheckpointHeader>, WaitUntilError>
    where
        F: Fn(&HashMap<NodeIndex, Option<AggregateCheckpointHeader>>) -> bool,
    {
        const DELAY: Duration = Duration::from_millis(100);

        wait_until(
            || async {
                let header_by_node = self
                    .node_by_id
                    .iter()
                    .map(|(node_id, node)| {
                        let query = node.checkpointer.query();
                        let header = query.get_aggregate_checkpoint_header(epoch);

                        (*node_id, header)
                    })
                    .collect::<HashMap<_, _>>();

                if condition(&header_by_node) {
                    Some(
                        header_by_node
                            .into_iter()
                            .map(|(node_id, header)| (node_id, header.unwrap()))
                            .collect::<HashMap<_, _>>(),
                    )
                } else {
                    None
                }
            },
            timeout,
            DELAY,
        )
        .await
    }

    /// Verify the signature of an aggregate checkpoint header.
    pub fn verify_aggregate_checkpointer_header(
        &self,
        agg_header: AggregateCheckpointHeader,
        node_id: NodeIndex,
        headers_by_node: HashMap<NodeIndex, HashMap<NodeIndex, CheckpointHeader>>,
    ) -> Result<bool> {
        // Get public keys of all nodes in the aggregate header.
        let mut pks = agg_header
            .nodes
            .iter()
            .map(|node_id| (node_id, self.node(node_id as u32).keystore.get_bls_pk()))
            .collect::<Vec<_>>();
        pks.sort_by_key(|(node_id, _)| *node_id);
        let pks = pks.into_iter().map(|(_, pk)| pk).collect::<Vec<_>>();

        // Get checkpoint header attestations for each node from the database.
        let mut attestations = headers_by_node[&node_id]
            .values()
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
