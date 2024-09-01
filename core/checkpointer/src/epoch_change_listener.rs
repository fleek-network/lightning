use std::marker::PhantomData;

use anyhow::Result;
use atomo::{DefaultSerdeBackend, SerdeBackend};
use fleek_crypto::SecretKey;
use lightning_interfaces::prelude::*;
use lightning_interfaces::types::CheckpointHeader;
use lightning_interfaces::EpochChangedNotification;
use ready::tokio::TokioReadyWaiter;
use ready::ReadyWaiter;
use tokio::task::JoinHandle;
use types::NodeIndex;

use crate::database::CheckpointerDatabase;
use crate::message::CheckpointBroadcastMessage;
use crate::rocks::RocksCheckpointerDatabase;

/// The epoch change listener is responsible for detecting when the current epoch has
/// changed and then broadcasting a checkpoint attestation for the new epoch.
pub struct EpochChangeListener<C: Collection> {
    _collection: PhantomData<C>,
}

impl<C: Collection> EpochChangeListener<C> {
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
        shutdown: ShutdownWaiter,
    ) -> (JoinHandle<Result<()>>, TokioReadyWaiter<()>) {
        let task = Task::<C>::new(node_id, db, keystore, pubsub, notifier);
        let ready = TokioReadyWaiter::<()>::new();
        let ready_sender = ready.clone();

        let handle = spawn!(
            async move {
                shutdown
                    .run_until_shutdown(task.start(ready_sender))
                    .await
                    .unwrap_or(Ok(()))
            },
            "CHECKPOINTER: epoch change listener"
        );

        (handle, ready)
    }
}
struct Task<C: Collection> {
    node_id: NodeIndex,
    db: RocksCheckpointerDatabase,
    keystore: C::KeystoreInterface,
    pubsub: c!(C::BroadcastInterface::PubSub<CheckpointBroadcastMessage>),
    notifier: C::NotifierInterface,
}

impl<C: Collection> Task<C> {
    pub fn new(
        node_id: NodeIndex,
        db: RocksCheckpointerDatabase,
        keystore: C::KeystoreInterface,
        pubsub: c!(C::BroadcastInterface::PubSub<CheckpointBroadcastMessage>),
        notifier: C::NotifierInterface,
    ) -> Self {
        Self {
            node_id,
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
        // Build our checkpoint attestation for the new epoch.
        let signer = self.keystore.get_bls_sk();
        let mut attestation = CheckpointHeader {
            epoch: epoch_changed.current_epoch,
            node_id: self.node_id,
            previous_state_root: epoch_changed.previous_state_root,
            next_state_root: epoch_changed.new_state_root,
            serialized_state_digest: epoch_changed.last_epoch_hash,
            signature: Default::default(),
        };
        let serialized_attestation = DefaultSerdeBackend::serialize(&attestation);
        attestation.signature = signer.sign(serialized_attestation.as_slice());

        // Broadcast our checkpoint attestation to the network.
        self.pubsub
            .send(
                &CheckpointBroadcastMessage::CheckpointHeader(attestation.clone()),
                None,
            )
            .await?;

        // Save our own checkpoint attestation to the database.
        self.db
            .add_checkpoint_header(epoch_changed.current_epoch, attestation);

        Ok(())
    }
}
