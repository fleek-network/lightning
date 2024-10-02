use std::collections::HashMap;
use std::time::Duration;

use anyhow::Result;
use atomo::{DefaultSerdeBackend, SerdeBackend};
use fleek_crypto::{ConsensusSignature, PublicKey};
use lightning_checkpointer::CheckpointBroadcastMessage;
use lightning_interfaces::prelude::*;
use lightning_interfaces::types::{
    AggregateCheckpoint,
    CheckpointAttestation,
    Epoch,
    NodeIndex,
    Topic,
};
use lightning_utils::poll::{poll_until, PollUntilError};

use super::TestNetwork;

impl TestNetwork {
    /// Send a checkpoint attestation to a specific node via their broadcaster pubsub.
    pub async fn broadcast_checkpoint_attestation_via_node(
        &self,
        node_id: NodeIndex,
        header: CheckpointAttestation,
    ) -> Result<()> {
        self.node(node_id)
            .broadcast
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
        &self,
        epoch: Epoch,
        condition: F,
    ) -> Result<HashMap<NodeIndex, HashMap<NodeIndex, CheckpointAttestation>>, PollUntilError>
    where
        F: Fn(&HashMap<NodeIndex, HashMap<NodeIndex, CheckpointAttestation>>) -> bool,
    {
        const TIMEOUT: Duration = Duration::from_secs(10);

        self.wait_for_checkpoint_attestations_with_timeout(epoch, condition, TIMEOUT)
            .await
    }

    pub async fn wait_for_checkpoint_attestations_with_timeout<F>(
        &self,
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
                let headers_by_node = self
                    .node_by_id
                    .iter()
                    .map(|(node_id, node)| {
                        let query = node.checkpointer.query();
                        let headers = query.get_checkpoint_attestations(epoch);

                        (*node_id, headers)
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
        &self,
        header: CheckpointAttestation,
    ) -> Result<bool> {
        let header_node = self.node(header.node_id);
        Ok(header_node.keystore.get_bls_pk().verify(
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
        &self,
        epoch: Epoch,
        condition: F,
    ) -> Result<HashMap<NodeIndex, AggregateCheckpoint>, PollUntilError>
    where
        F: Fn(&HashMap<NodeIndex, Option<AggregateCheckpoint>>) -> bool,
    {
        const TIMEOUT: Duration = Duration::from_secs(10);

        self.wait_for_aggregate_checkpoint_with_timeout(epoch, condition, TIMEOUT)
            .await
    }

    pub async fn wait_for_aggregate_checkpoint_with_timeout<F>(
        &self,
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
                let header_by_node = self
                    .node_by_id
                    .iter()
                    .map(|(node_id, node)| {
                        let query = node.checkpointer.query();
                        let header = query.get_aggregate_checkpoint(epoch);

                        (*node_id, header)
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
        &self,
        agg_header: AggregateCheckpoint,
        node_id: NodeIndex,
        headers_by_node: HashMap<NodeIndex, HashMap<NodeIndex, CheckpointAttestation>>,
    ) -> Result<bool> {
        // Get public keys of all nodes in the aggregate header.
        let mut pks = agg_header
            .nodes
            .iter()
            .map(|node_id| (node_id, self.node(node_id as u32).keystore.get_bls_pk()))
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
}
