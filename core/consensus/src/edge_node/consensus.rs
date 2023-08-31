use std::sync::Arc;

use fleek_crypto::NodePublicKey;
use lightning_interfaces::{BroadcastEventInterface, PubSub, SyncQueryRunnerInterface, ToDigest};
use log::info;
use tokio::pin;
use tokio::sync::{mpsc, Notify};
use tokio::task::JoinHandle;

use super::transaction_store::TransactionStore;
use crate::consensus::PubSubMsg;
use crate::execution::{AuthenticStampedParcel, CommitteeAttestation, Execution};

pub struct EdgeConsensus {
    handle: JoinHandle<()>,
    tx_shutdown: Arc<Notify>,
}

impl EdgeConsensus {
    pub fn spawn<P: PubSub<PubSubMsg> + 'static, Q: SyncQueryRunnerInterface>(
        pub_sub: P,
        execution: Arc<Execution>,
        reconfigure_notify: Arc<Notify>,
        query_runner: Q,
        node_public_key: NodePublicKey,
        rx_narwhal_batches: mpsc::Receiver<AuthenticStampedParcel>,
    ) -> Self {
        let shutdown_notify = Arc::new(Notify::new());

        let handle = tokio::spawn(message_receiver_worker(
            pub_sub,
            shutdown_notify.clone(),
            execution,
            query_runner,
            node_public_key,
            reconfigure_notify,
            rx_narwhal_batches,
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
async fn message_receiver_worker<P: PubSub<PubSubMsg>, Q: SyncQueryRunnerInterface>(
    mut pub_sub: P,
    shutdown_notify: Arc<Notify>,
    execution: Arc<Execution>,
    query_runner: Q,
    node_public_key: NodePublicKey,
    reconfigure_notify: Arc<Notify>,
    mut rx_narwhal_batch: mpsc::Receiver<AuthenticStampedParcel>,
) {
    info!("Edge node message worker is running");
    let mut committee = query_runner.get_committee_members_by_index();
    let mut quorom_threshold = (committee.len() * 2) / 3 + 1;
    let mut our_index = query_runner
        .pubkey_to_index(node_public_key)
        .unwrap_or(u32::MAX);
    let mut on_committee = committee.contains(&our_index);

    let mut transaction_store = TransactionStore::new();
    loop {
        // todo(dalton): revisit pinning these and using Notify over oneshot
        let reconfigure_future = reconfigure_notify.notified();
        let shutdown_future = shutdown_notify.notified();
        pin!(shutdown_future);
        pin!(reconfigure_future);

        tokio::select! {
            _ = shutdown_future => {
                return;
            },
            Some(parcel) = rx_narwhal_batch.recv() => {
                if !on_committee {
                    // This should never happen if it somehow does there is critical error somewhere
                    panic!("We somehow sent ourselves a parcel from narwhal while not on committee");
                }
                transaction_store.store_parcel(parcel.clone());
                // No need to store the attestation we have already executed it

                let parcel_digest = parcel.to_digest();

                transaction_store.set_head(parcel_digest);

                let attestation = CommitteeAttestation {
                    digest: parcel_digest,
                    node_index: our_index
                };

                info!("Send transaction parcel to broadcast as a validator");
                pub_sub.send(&attestation.into()).await;
                pub_sub.send(&parcel.into()).await;
            },
            _ = reconfigure_future => {
               committee = query_runner.get_committee_members_by_index();
               quorom_threshold = (committee.len() * 2) / 3 + 1;
               // We recheck our index incase it was non existant before and we staked during this epoch and finally got the certificate
               our_index = query_runner
                .pubkey_to_index(node_public_key)
                .unwrap_or(u32::MAX);
               on_committee = committee.contains(&our_index);
            },
            Some(mut msg) = pub_sub.recv_event() => {
                if on_committee {
                    msg.propagate();
                } else {
                    match msg.take().unwrap() {
                        PubSubMsg::Transactions(parcel) => {
                        info!("Received transaction parcel from gossip as an edge node");
                        if !committee.contains(&msg.originator()){
                            msg.mark_invalid_sender();
                            continue;
                        }
                        let parcel_digest = parcel.to_digest();

                        transaction_store.store_parcel(parcel);

                        transaction_store
                        .try_execute(parcel_digest,quorom_threshold,&execution).await;


                    },
                    PubSubMsg::Attestation(att) => {

                        info!("Received parcel attestation from gossip as an edge node");
                        let originator = msg.originator();

                        if originator != att.node_index || !committee.contains(&originator){
                            msg.mark_invalid_sender();
                            continue;
                        }

                        if !on_committee{
                            transaction_store.add_attestation(att.digest, att.node_index);
                            transaction_store
                            .try_execute(att.digest, quorom_threshold, &execution).await;
                        }
                        msg.propagate();
                    }
                }
                }

            },

        }
    }
}
