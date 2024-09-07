use anyhow::{Context, Result};
use atomo::{DefaultSerdeBackend, SerdeBackend};
use fleek_crypto::{ConsensusPublicKey, ConsensusSignature, PublicKey};
use lightning_interfaces::prelude::*;
use lightning_interfaces::types::CheckpointHeader;
use tokio::task::JoinHandle;

use crate::aggregate_builder::AggregateCheckpointBuilder;
use crate::database::CheckpointerDatabase;
use crate::message::CheckpointBroadcastMessage;
use crate::rocks::RocksCheckpointerDatabase;

/// The attestation listener is responsible for listening for checkpoint attestation
/// messages and saving them to the local database.
///
/// When a supermajority of attestations for epochs are consistent, it aggregates the BLS
/// signatures to create a canonical aggregate checkpoint header, which is saves to the local
/// database for sharing with other nodes and clients in the future.
pub struct AttestationListener<C: Collection> {
    db: RocksCheckpointerDatabase,
    aggregate: AggregateCheckpointBuilder<C>,
    pubsub: c!(C::BroadcastInterface::PubSub<CheckpointBroadcastMessage>),
    app_query: c!(C::ApplicationInterface::SyncExecutor),
}

impl<C: Collection> AttestationListener<C> {
    pub fn new(
        db: RocksCheckpointerDatabase,
        pubsub: c!(C::BroadcastInterface::PubSub<CheckpointBroadcastMessage>),
        app_query: c!(C::ApplicationInterface::SyncExecutor),
    ) -> Self {
        Self {
            aggregate: AggregateCheckpointBuilder::new(db.clone(), app_query.clone()),
            db,
            pubsub,
            app_query,
        }
    }

    /// Spawn task for and start the attestation listener.
    ///
    /// This method spawns a new task and returns immediately. It does not block
    /// until the task is complete.
    pub fn spawn(self, shutdown: ShutdownWaiter) -> JoinHandle<()> {
        let waiter = shutdown.clone();
        spawn!(
            async move {
                waiter
                    .run_until_shutdown(self.start())
                    .await
                    .unwrap_or(Ok(())) // Shutdown was triggered, so we return Ok(())
                    .context("attestation listener task failed")
                    .unwrap()
            },
            "CHECKPOINTER: attestation listener",
            crucial(shutdown)
        )
    }

    // Start the attestation listener, listening for incoming checkpoint attestation messages from
    // the broadcaster pubsub topic.
    pub async fn start(mut self) -> Result<()> {
        tracing::debug!("starting attestation listener");

        loop {
            tokio::select! {
                Some(msg) = self.pubsub.recv() => {
                    tracing::debug!("received checkpoint attestation message: {:?}", msg);
                    match msg {
                        CheckpointBroadcastMessage::CheckpointHeader(checkpoint_header) => {
                            self.handle_incoming_checkpoint_header(checkpoint_header)?;
                        }
                    }
                }
                else => {
                    tracing::debug!("broadcast subscription is closed");
                    break;
                }
            }
        }

        tracing::debug!("shutdown attestation listener");
        Ok(())
    }

    fn handle_incoming_checkpoint_header(
        &mut self,
        checkpoint_header: CheckpointHeader,
    ) -> Result<()> {
        let epoch = checkpoint_header.epoch;
        // Ignore if from node that is not in the eligible node set.
        let nodes = self.aggregate.get_eligible_nodes();
        if !nodes.contains_key(&checkpoint_header.node_id) {
            tracing::debug!(
                "ignoring incoming checkpoint header for epoch {}, node not in eligible node set",
                epoch
            );
            return Ok(());
        }

        // Get the node's consensus BLS public key.
        let node_consensus_key = match self
            .app_query
            .get_node_info(&checkpoint_header.node_id, |node| node.consensus_key)
        {
            Some(key) => key,
            None => {
                tracing::warn!(
                    "checkpointer header node {} not found",
                    checkpoint_header.node_id
                );
                return Ok(());
            },
        };

        // Validate the incoming checkpoint header; ignore if invalid.
        if let Err(e) = self.validate_checkpoint_header(&checkpoint_header, node_consensus_key) {
            tracing::info!(
                "ignoring incoming checkpoint header for epoch {}, invalid signature: {:?}",
                epoch,
                e
            );
            return Ok(());
        }

        // Save the incoming checkpoint header attestation to the database.
        self.db.set_node_checkpoint_header(epoch, checkpoint_header);

        // If there is a supermajority of eligible nodes in agreement, build and save an aggregate
        // checkpoint header.
        self.aggregate
            .build_and_save_aggregate_if_supermajority(epoch, nodes.len())?;

        Ok(())
    }

    fn validate_checkpoint_header(
        &self,
        header: &CheckpointHeader,
        node_consensus_key: ConsensusPublicKey,
    ) -> Result<()> {
        let serialized_signed_header = DefaultSerdeBackend::serialize(&CheckpointHeader {
            signature: ConsensusSignature::default(),
            ..header.clone()
        });
        if !node_consensus_key.verify(&header.signature, &serialized_signed_header)? {
            return Err(anyhow::anyhow!("invalid checkpoint header signature"));
        }

        Ok(())
    }
}
