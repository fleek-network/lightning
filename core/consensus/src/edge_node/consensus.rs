use std::collections::HashSet;
use std::sync::Arc;

use fleek_crypto::NodePublicKey;
use lightning_interfaces::{BroadcastEventInterface, PubSub, SyncQueryRunnerInterface, ToDigest};
use tokio::pin;
use tokio::sync::{mpsc, Notify};
use tokio::task::JoinHandle;
use tracing::info;

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
        execution: Arc<Execution<Q>>,
        query_runner: Q,
        node_public_key: NodePublicKey,
        rx_narwhal_batches: mpsc::Receiver<(AuthenticStampedParcel, bool)>,
        reconfigure_notify: Arc<Notify>,
    ) -> Self {
        let shutdown_notify = Arc::new(Notify::new());

        let handle = tokio::spawn(message_receiver_worker(
            pub_sub,
            shutdown_notify.clone(),
            execution,
            query_runner,
            node_public_key,
            rx_narwhal_batches,
            reconfigure_notify,
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
    execution: Arc<Execution<Q>>,
    query_runner: Q,
    node_public_key: NodePublicKey,
    mut rx_narwhal_batch: mpsc::Receiver<(AuthenticStampedParcel, bool)>,
    reconfigure_notify: Arc<Notify>,
) {
    info!("Edge node message worker is running");
    let mut committee = query_runner.get_committee_members_by_index();
    let mut quorom_threshold = (committee.len() * 2) / 3 + 1;
    let mut our_index = query_runner
        .pubkey_to_index(node_public_key)
        .unwrap_or(u32::MAX);
    let mut on_committee = committee.contains(&our_index);

    let mut txn_store = TransactionStore::new();
    loop {
        // todo(dalton): revisit pinning these and using Notify over oneshot
        let shutdown_future = shutdown_notify.notified();
        pin!(shutdown_future);

        tokio::select! {
            _ = shutdown_future => {
                return;
            },
            Some((parcel, epoch_changed)) = rx_narwhal_batch.recv() => {
                if !on_committee {
                    // This should never happen if it somehow does there is critical error somewhere
                    panic!("We somehow sent ourselves a parcel from narwhal while not on committee");
                }

                let parcel_digest = parcel.to_digest();

                txn_store.store_parcel(parcel.clone(), None);
                // No need to store the attestation we have already executed it

                let attestation = CommitteeAttestation {
                    digest: parcel_digest,
                    node_index: our_index
                };

                info!("Send transaction parcel to broadcast as a validator");
                pub_sub.send(&attestation.into(), None).await;
                pub_sub.send(&parcel.into(), None).await;

                if epoch_changed {
                    committee = query_runner.get_committee_members_by_index();
                    quorom_threshold = (committee.len() * 2) / 3 + 1;
                    // We recheck our index incase it was non existant before
                    // and we staked during this epoch and finally got the certificate
                    our_index = query_runner
                        .pubkey_to_index(node_public_key)
                        .unwrap_or(u32::MAX);
                    on_committee = committee.contains(&our_index);
                }
            },
            Some(mut msg) = pub_sub.recv_event() => {
                match msg.take().unwrap() {
                    PubSubMsg::Transactions(parcel) => {
                        if !committee.contains(&msg.originator()){
                            msg.mark_invalid_sender();
                            continue;
                        }

                        let msg_digest = msg.get_digest();
                        // propagate here now that we know its good and before we do async work
                        msg.propagate();

                        let parcel_digest = parcel.to_digest();

                        txn_store.store_parcel(parcel, Some(msg_digest));

                        if !on_committee {
                            info!("Received transaction parcel from gossip as an edge node");
                            // get the current chain head
                            let head = query_runner.get_last_block();

                            match txn_store
                            .try_execute(parcel_digest,quorom_threshold,&execution,head).await {
                                Ok(epoch_changed) => {
                                    if epoch_changed {
                                        committee = query_runner.get_committee_members_by_index();
                                        quorom_threshold = (committee.len() * 2) / 3 + 1;
                                        // We recheck our index incase it was non existant before and
                                        // we staked during this epoch and finally got the certificate
                                        our_index = query_runner
                                            .pubkey_to_index(node_public_key)
                                            .unwrap_or(u32::MAX);
                                        on_committee = committee.contains(&our_index);
                                        reconfigure_notify.notify_waiters();
                                    }
                                }
                                Err(_not_executed) => {

                                }
                            }
                        }
                    },
                    PubSubMsg::Attestation(att) => {
                        let originator = msg.originator();

                        if originator != att.node_index || !committee.contains(&originator){
                            msg.mark_invalid_sender();
                            continue;
                        }

                        // propagate msg as soon as we know its good
                        msg.propagate();

                        if !on_committee{
                            info!("Received parcel attestation from gossip as an edge node");
                            txn_store.add_attestation(
                                att.digest,
                                att.node_index,
                            );

                            // get the current chain head
                            let head = query_runner.get_last_block();

                            match txn_store
                            .try_execute(att.digest, quorom_threshold, &execution, head).await {
                                Ok(epoch_changed) => {
                                    if epoch_changed {
                                        committee = query_runner
                                            .get_committee_members_by_index();
                                        quorom_threshold = (committee.len() * 2) / 3 + 1;
                                        // We recheck our index incase it was non existant before and
                                        // we staked during this epoch and finally got the certificate
                                        our_index = query_runner
                                            .pubkey_to_index(node_public_key)
                                            .unwrap_or(u32::MAX);
                                        on_committee = committee.contains(&our_index);
                                        reconfigure_notify.notify_waiters();
                                    }
                                }
                                Err(_not_executed) => {}
                            }
                        }
                    }
                    PubSubMsg::RequestTransactions(digest) => {
                        // TODO(matthias): should we propagate requests?
                        //msg.propagate();
                        if let Some(parcel) = txn_store.get_parcel(&digest) {
                            if let Some(msg_digest) = parcel.message_digest {
                                let filter = HashSet::from_iter(vec![msg.originator()].into_iter());
                                pub_sub.repropagate(msg_digest, Some(filter)).await;
                            }
                        }
                    }
                }

            },
        }
    }
}
