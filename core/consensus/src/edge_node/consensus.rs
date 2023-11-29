use std::collections::HashSet;
use std::sync::Arc;

use fleek_crypto::NodePublicKey;
use lightning_interfaces::types::Epoch;
use lightning_interfaces::{BroadcastEventInterface, PubSub, SyncQueryRunnerInterface, ToDigest};
use tokio::pin;
use tokio::sync::{mpsc, Notify};
use tokio::task::JoinHandle;
use tracing::info;

use super::transaction_store::{NotExecuted, TransactionStore};
use crate::consensus::PubSubMsg;
use crate::execution::{AuthenticStampedParcel, CommitteeAttestation, Digest, Execution};

const MAX_PENDING_TIMEOUTS: usize = 10;

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
    let (timeout_tx, mut timeout_rx) = mpsc::channel(128);
    let mut pending_timeouts = HashSet::new();

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

                txn_store.store_parcel(parcel.clone(), our_index, None);
                // No need to store the attestation we have already executed it

                let attestation = CommitteeAttestation {
                    digest: parcel_digest,
                    node_index: our_index,
                    epoch: parcel.epoch,
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
                        let epoch = query_runner.get_epoch();
                        let originator = msg.originator();
                        let is_committee = committee.contains(&originator);
                        if !is_valid_message(is_committee, parcel.epoch, epoch) {
                            msg.mark_invalid_sender();
                            continue;
                        }

                        let msg_digest = msg.get_digest();
                        // propagate here now that we know its good and before we do async work
                        msg.propagate();

                        let parcel_digest = parcel.to_digest();

                        if parcel.epoch == epoch + 1 {
                            // if the parcel is from the next epoch, we optimistically
                            // store it and check if it's from a validator once we change
                            // epochs
                            txn_store.store_pending_parcel(parcel, originator, msg_digest);
                        } else {
                            txn_store.store_parcel(parcel, originator, Some(msg_digest));
                        }

                        if !on_committee {
                            info!("Received transaction parcel from gossip as an edge node");

                            match txn_store
                            .try_execute(
                                parcel_digest,
                                quorom_threshold,
                                &query_runner,
                                &execution,
                            ).await {
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

                                        // Check the validity of the parcels/attestations that we
                                        // stored optimistically.
                                        txn_store.change_epoch(&committee);
                                    }
                                }
                                Err(not_executed) => {
                                    handle_not_executed(
                                        not_executed,
                                        &txn_store,
                                        timeout_tx.clone(),
                                        &mut pending_timeouts,
                                    ).await;
                                }
                            }
                        }
                    },
                    PubSubMsg::Attestation(att) => {
                        let originator = msg.originator();

                        let epoch = query_runner.get_epoch();
                        let is_committee = committee.contains(&originator);
                        if originator != att.node_index
                            || !is_valid_message(is_committee, att.epoch, epoch) {
                            msg.mark_invalid_sender();
                            continue;
                        }

                        // propagate msg as soon as we know its good
                        msg.propagate();

                        if !on_committee {
                            info!("Received parcel attestation from gossip as an edge node");

                            if att.epoch == epoch + 1 {
                                // if the attestation is from the next epoch, we optimistically
                                // store it and check if it's from a validator once we change
                                // epochs
                                txn_store.add_pending_attestation(att.digest, att.node_index);
                            } else {
                                txn_store.add_attestation(
                                    att.digest,
                                    att.node_index,
                                );
                            }

                            match txn_store
                            .try_execute(
                                att.digest,
                                quorom_threshold,
                                &query_runner,
                                &execution,
                            ).await {
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

                                        // Check the validity of the parcels/attestations that we
                                        // stored optimistically.
                                        txn_store.change_epoch(&committee);
                                    }
                                }
                                Err(not_executed) => {
                                    handle_not_executed(
                                        not_executed,
                                        &txn_store,
                                        timeout_tx.clone(),
                                        &mut pending_timeouts,
                                    ).await;
                                }
                            }
                        }
                    }
                    PubSubMsg::RequestTransactions(digest) => {
                        // TODO(matthias): should we propagate requests?
                        //msg.propagate();
                        if let Some(parcel) = txn_store.get_parcel(&digest) {
                            if let Some(msg_digest) = parcel.message_digest {
                                let filter = HashSet::from([msg.originator()]);
                                pub_sub.repropagate(msg_digest, Some(filter)).await;
                            }
                        }
                    }
                }

            },
            digest = timeout_rx.recv() => {
                // Timeout for a missing parcel. If we still haven't received the parcel, we send a
                // request.
                if let Some(digest) = digest {
                    pending_timeouts.remove(&digest);
                    if txn_store.get_parcel(&digest).is_none() {
                        let request = PubSubMsg::RequestTransactions(digest);
                        pub_sub.send(&request, None).await;
                    }
                }
            }
        }
    }
}

// While trying to connect the chain back to the head, we discovered a missing parcel.
// This is can happen normally, because parcels or attestations might arrive out of
// order.
// However, this could also mean that we missed a broadcast message containing a parcel,
// and our peers are no longer broadcasting this message.
// In this case we want to send a request out for this parcel.
// In order to prevent sending out these requests prematurely, we keep a running average of
// the intervals between executing parcels.
// If we are missing a parcel, and the time that has passed since trying toexecute the last parcel
// is larger than the expected time, we send out a request.
async fn handle_not_executed(
    not_executed: NotExecuted,
    txn_store: &TransactionStore,
    timeout_tx: mpsc::Sender<Digest>,
    pending_timeouts: &mut HashSet<Digest>,
) {
    if let NotExecuted::MissingParcel(digest) = not_executed {
        if !pending_timeouts.contains(&digest) && pending_timeouts.len() < MAX_PENDING_TIMEOUTS {
            let timeout = txn_store.get_timeout();
            tokio::spawn(async move {
                tokio::time::sleep(timeout).await;
                let _ = timeout_tx.send(digest).await;
            });
        }
        pending_timeouts.insert(digest);
    }
}

// The parcel must be either from the current committee or from the
// next epoch. We optimistically accept parcels from the following
// epoch in case we missed the epoch change parcel. We will verify
// later on that the parcel was sent from a committee member.
fn is_valid_message(in_committee: bool, msg_epoch: Epoch, current_epoch: Epoch) -> bool {
    (in_committee && msg_epoch == current_epoch) || msg_epoch == current_epoch + 1
}

#[cfg(test)]
mod tests {
    use crate::edge_node::consensus::is_valid_message;

    #[test]
    fn test_is_valid_message() {
        // msg is from a committee member, msg epoch is the current epoch => valid
        assert!(is_valid_message(true, 2, 2));
        // msg is from a committee member, msg epoch is from the next epoch => valid
        assert!(is_valid_message(true, 4, 3));
        // msg is from a committee member, msg epoch is from the current epoch + 2 => invalid
        assert!(!is_valid_message(true, 7, 5));
        // msg is from a committee member, msg epoch is from the last epoch => invalid
        assert!(!is_valid_message(true, 4, 5));
        // msg is not from a committee member, msg epoch is the next epoch => valid
        assert!(is_valid_message(false, 2, 1));
        // msg is not from a committee member, msg epoch is the current epoch + 2 => invalid
        assert!(!is_valid_message(false, 3, 1));
        // msg is not from a committee member, msg epoch is the last epoch => invalid
        assert!(!is_valid_message(false, 1, 2));
    }
}
