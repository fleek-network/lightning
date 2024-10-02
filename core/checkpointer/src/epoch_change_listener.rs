use std::marker::PhantomData;

use anyhow::{Context, Result};
use atomo::{DefaultSerdeBackend, SerdeBackend};
use fleek_crypto::SecretKey;
use lightning_interfaces::prelude::*;
use lightning_interfaces::types::CheckpointAttestation;
use lightning_interfaces::EpochChangedNotification;
use ready::tokio::TokioReadyWaiter;
use ready::ReadyWaiter;
use tokio::task::JoinHandle;
use types::NodeIndex;

use crate::aggregate_builder::AggregateCheckpointBuilder;
use crate::database::{CheckpointerDatabase, CheckpointerDatabaseQuery};
use crate::message::CheckpointBroadcastMessage;
use crate::rocks::RocksCheckpointerDatabase;

/// The epoch change listener is responsible for detecting when the current epoch has
/// changed and then broadcasting a checkpoint attestation for the new epoch.
///
/// The epoch change listener also needs to build and save an aggregate checkpoint for the new
/// epoch if a supermajority of eligible nodes have attested to the checkpoint attestation for the
/// epoch. We need to do this here as well because other nodes may have already received their
/// epoch change notification and broadcasted checkpoint attestations for the new epoch.
pub struct EpochChangeListener<C: NodeComponents> {
    _components: PhantomData<C>,
}

impl<C: NodeComponents> EpochChangeListener<C> {
    /// Spawn task starting the epoch change listener.
    ///
    /// This method spawns a new task and returns immediately. It does not block
    /// until the task is complete or even ready.
    ///
    /// Returns a handle to the spawned task and a ready waiter. The caller can
    /// call `ready.wait()` to block until the task is ready.
    pub fn spawn(
        node_id: NodeIndex,
        db: RocksCheckpointerDatabase,
        keystore: C::KeystoreInterface,
        pubsub: c!(C::BroadcastInterface::PubSub<CheckpointBroadcastMessage>),
        notifier: C::NotifierInterface,
        app_query: c!(C::ApplicationInterface::SyncExecutor),
        shutdown: ShutdownWaiter,
    ) -> (JoinHandle<()>, TokioReadyWaiter<()>) {
        let task = Task::<C>::new(node_id, db, keystore, pubsub, notifier, app_query);
        let ready = TokioReadyWaiter::<()>::new();
        let ready_sender = ready.clone();

        let waiter = shutdown.clone();
        let handle = spawn!(
            async move {
                waiter
                    .run_until_shutdown(task.start(ready_sender))
                    .await
                    .unwrap_or(Ok(())) // Shutdown was triggered, so we return Ok(())
                    .context("epoch change listener task failed")
                    .unwrap()
            },
            "CHECKPOINTER: epoch change listener",
            crucial(shutdown)
        );

        (handle, ready)
    }
}
struct Task<C: NodeComponents> {
    node_id: NodeIndex,
    db: RocksCheckpointerDatabase,
    aggregate: AggregateCheckpointBuilder<C>,
    keystore: C::KeystoreInterface,
    pubsub: c!(C::BroadcastInterface::PubSub<CheckpointBroadcastMessage>),
    notifier: C::NotifierInterface,
}

impl<C: NodeComponents> Task<C> {
    pub fn new(
        node_id: NodeIndex,
        db: RocksCheckpointerDatabase,
        keystore: C::KeystoreInterface,
        pubsub: c!(C::BroadcastInterface::PubSub<CheckpointBroadcastMessage>),
        notifier: C::NotifierInterface,
        app_query: c!(C::ApplicationInterface::SyncExecutor),
    ) -> Self {
        Self {
            node_id,
            aggregate: AggregateCheckpointBuilder::new(db.clone(), app_query.clone()),
            db,
            keystore,
            pubsub,
            notifier,
        }
    }

    pub async fn start(&self, ready: TokioReadyWaiter<()>) -> Result<()> {
        tracing::debug!("starting epoch change listener");

        let mut epoch_changed_sub = self.notifier.subscribe_epoch_changed();

        // Notify that we are ready and subscribed to the epoch changed notifier.
        ready.notify(());

        loop {
            tokio::select! {
                Some(epoch_changed) = epoch_changed_sub.recv() => {
                    tracing::debug!("received epoch changed notification: {:?}", epoch_changed);
                    self.handle_epoch_changed(epoch_changed).await?;
                },
                else => {
                    tracing::debug!("notifier subscription is closed");
                    break;
                }
            }
        }

        tracing::debug!("epoch change listener shutdown");
        Ok(())
    }

    async fn handle_epoch_changed(&self, epoch_changed: EpochChangedNotification) -> Result<()> {
        let epoch = epoch_changed.current_epoch;

        // Ignore if we're not in the eligible set of nodes.
        let nodes = self.aggregate.get_eligible_nodes();
        if !nodes.contains_key(&self.node_id) {
            tracing::debug!(
                "ignoring epoch changed notification for epoch {}, node not in eligible set",
                epoch
            );
            return Ok(());
        }

        // Ignore if a checkpoint attestation for this epoch from the same node already exists.
        if self
            .db
            .query()
            .get_node_checkpoint_attestation(epoch, self.node_id)
            .is_some()
        {
            tracing::debug!(
                "ignoring epoch changed notification for epoch {}, node already has checkpoint attestation",
                epoch
            );
            return Ok(());
        }

        // Build our checkpoint attestation for the new epoch.
        // Build our checkpoint attestation for the new epoch.
        let signer = self.keystore.get_bls_sk();
        let mut attestation = CheckpointAttestation {
            epoch,
            node_id: self.node_id,
            previous_state_root: epoch_changed.previous_state_root,
            next_state_root: epoch_changed.new_state_root,
            serialized_state_digest: epoch_changed.last_epoch_hash,
            signature: Default::default(),
        };
        let serialized_attestation = DefaultSerdeBackend::serialize(&attestation);
        attestation.signature = signer.sign(serialized_attestation.as_slice());

        // Save our own checkpoint attestation to the database.
        self.db
            .set_node_checkpoint_attestation(epoch, attestation.clone());

        // Broadcast our checkpoint attestation to the network.
        self.pubsub
            .send(
                &CheckpointBroadcastMessage::CheckpointAttestation(attestation),
                None,
            )
            .await?;

        // Check for supermajority of checkpoint attestations for the epoch, in case we have already
        // received the checkpoint attestations from other nodes or if we're the only node.
        self.aggregate
            .build_and_save_aggregate_if_supermajority(epoch, nodes.len())?;

        Ok(())
    }
}
