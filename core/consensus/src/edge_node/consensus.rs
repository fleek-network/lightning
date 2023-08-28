use std::sync::Arc;

use lightning_interfaces::{PubSub, ToDigest};
use log::info;
use narwhal_config::Committee;
use tokio::sync::Notify;
use tokio::task::JoinHandle;

use super::transaction_store::TransactionStore;
use crate::consensus::PubSubMsg;
use crate::execution::Execution;

pub struct EdgeConsensus {
    handle: JoinHandle<()>,
    tx_shutdown: Arc<Notify>,
}

impl EdgeConsensus {
    pub fn spawn<P: PubSub<PubSubMsg> + 'static>(
        pub_sub: P,
        committee: Committee,
        execution: Arc<Execution<P>>,
    ) -> Self {
        let txn_store = TransactionStore::new();
        let shutdown_notify = Arc::new(Notify::new());

        let quorom_threshold = (committee.size() * 2) / 3 + 1;

        let handle = tokio::spawn(message_receiver_worker(
            pub_sub,
            shutdown_notify.clone(),
            txn_store,
            execution,
            quorom_threshold,
        ));

        Self {
            handle,
            tx_shutdown: shutdown_notify,
        }
    }

    /// Consume this executor and shutdown all of the workers and processes.
    pub async fn shutdown(self) {
        // Send the shutdown signal.
        self.tx_shutdown.notify_waiters();

        // Gracefully wait for all the subtasks to finish and return.
        self.handle.await.unwrap();
    }
}

/// Creates and event loop which consumes messages from pubsub and sends them to the
/// right destination.
async fn message_receiver_worker<P: PubSub<PubSubMsg>>(
    mut pub_sub: P,
    shutdown_notify: Arc<Notify>,
    mut transaction_store: TransactionStore,
    execution: Arc<Execution<P>>,
    quorom_threshold: usize,
) {
    info!("Edge node message worker is running");
    loop {
        tokio::select! {
            _ = shutdown_notify.notified() => {
                return;
            },
            Some(msg) = pub_sub.recv() => {
                match msg {
                    PubSubMsg::Transactions(parcel) => {
                    info!("Received transaction parcel from gossip as an edge node");
                    // TODO(qti3e): The gossip recv should return the originator of the message
                    // so we can verify that it is a committee member here.
                    let parcel_digest = parcel.to_digest();

                    transaction_store.store_parcel(parcel);

                    transaction_store.try_execute(parcel_digest,quorom_threshold,&execution).await;

                    // Check if this ready to be committed
                },
                PubSubMsg::Attestation(att) => {
                    info!("Received parcel attestation from gossip as an edge node");
                    // todo() when gossip reciever returns originator make sure this member is a
                    // committee member

                    transaction_store.add_attestation(att.digest, att.node_index);
                    transaction_store.try_execute(att.digest, quorom_threshold, &execution).await;
                }
            }

            }

        }
    }
}
