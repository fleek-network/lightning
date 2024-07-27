use std::collections::{HashSet, VecDeque};
use std::sync::Arc;
use std::time::{Duration, SystemTime};

use fleek_crypto::NodePublicKey;
use lightning_interfaces::prelude::*;
use lightning_interfaces::types::{Block, Epoch, Metadata, NodeIndex, TransactionRequest};
use lightning_interfaces::Events;
use lightning_metrics::increment_counter;
use lightning_utils::application::QueryRunnerExt;
use narwhal_types::{Batch, ConsensusOutput, Transaction};
use quick_cache::unsync::Cache;
use tokio::pin;
use tokio::sync::mpsc::Receiver;
use tokio::sync::{mpsc, oneshot, Notify};
use tokio::task::JoinHandle;
use tracing::{error, info};

use super::parcel::{AuthenticStampedParcel, CommitteeAttestation, Digest};
use super::transaction_store::TransactionStore;
use crate::consensus::PubSubMsg;

const MAX_PENDING_TIMEOUTS: usize = 100;
// Exponentially moving average parameter for estimating the time between executions of parcels.
// This parameter must be in range [0, 1].
const TBE_EMA: f64 = 0.125;

// Bounds for the estimated time between parcel executions.
const MIN_TBE: Duration = Duration::from_secs(30);
const MAX_TBE: Duration = Duration::from_secs(40);

pub struct ExecutionWorker {
    handle: JoinHandle<()>,
    tx_shutdown: Arc<Notify>,
}

struct Context<P: PubSub<PubSubMsg>, Q: SyncQueryRunnerInterface, NE: Emitter> {
    /// Managing certificates generated by narwhal.
    executor: ExecutionEngineSocket,
    /// Store parcels and attestations.
    txn_store: TransactionStore<P::Event>,
    /// Pending parcel digests.
    pending_digests: HashSet<Digest>,
    /// Executed parcel digests.
    executed_digests: HashSet<Digest>,
    /// quorom threshold to reach consensus.
    quorom_threshold: usize,
    /// The current validator committee.
    committee: Vec<NodeIndex>,
    /// The node index of this node.
    our_index: NodeIndex,
    /// Whether or not this node is currently on the committee.
    on_committee: bool,
    /// The public key of this node.
    node_public_key: NodePublicKey,
    /// Pending timeouts for parcels.
    pending_timeouts: HashSet<Digest>,
    /// Pending requests for missing parcels.
    pending_requests: Cache<Digest, ()>,
    /// Query runner.
    query_runner: Q,
    /// Pubsub handle to send and receive broadcast messages.
    pub_sub: P,
    /// Send the event to the RPC
    event_tx: Events,
    /// Notifications emitter
    notifier: NE,
    /// Used for sending timeouts to the main tokio select loop.
    timeout_tx: mpsc::Sender<Digest>,
    /// Used to signal internal consensus processes that it is time to reconfigure for a new epoch
    reconfigure_notify: Arc<Notify>,
    /// The timestamp of the last executed parcel.
    last_executed_timestamp: Option<SystemTime>,
    /// The estimated time between parcel executions.
    estimated_tbe: Duration,
    /// The deviation for the time between parcel executions.
    deviation_tbe: Duration,
}

impl ExecutionWorker {
    #[allow(clippy::too_many_arguments)]
    pub fn spawn<P: PubSub<PubSubMsg> + 'static, Q: SyncQueryRunnerInterface, NE: Emitter>(
        executor: ExecutionEngineSocket,
        consensus_output_rx: Receiver<ConsensusOutput>,
        pub_sub: P,
        query_runner: Q,
        node_public_key: NodePublicKey,
        reconfigure_notify: Arc<Notify>,
        notifier: NE,
        event_tx_rx: oneshot::Receiver<Events>,
    ) -> Self {
        let shutdown_notify = Arc::new(Notify::new());

        let handle = spawn!(
            spawn_worker::<P, Q, NE>(
                executor,
                consensus_output_rx,
                pub_sub,
                shutdown_notify.clone(),
                query_runner,
                node_public_key,
                reconfigure_notify,
                notifier,
                event_tx_rx,
            ),
            "CONSENSUS: message receiver worker"
        );

        Self {
            handle,
            tx_shutdown: shutdown_notify,
        }
    }

    /// Consume this executor and shutdown all of the workers and processes.
    pub async fn shutdown(self) {
        // Send the shutdown signal.
        self.tx_shutdown.notify_one();

        // Gracefully wait for all the subtasks to finish and return.
        if let Err(e) = self.handle.await {
            error!(
                "Failed to join handle in file {} at line {}: {e}",
                file!(),
                line!()
            );
        }
    }
}

/// Creates and event loop which consumes messages from pubsub and sends them to the
/// right destination.
#[allow(clippy::too_many_arguments)]
async fn spawn_worker<P: PubSub<PubSubMsg>, Q: SyncQueryRunnerInterface, NE: Emitter>(
    executor: ExecutionEngineSocket,
    mut consensus_output_rx: Receiver<ConsensusOutput>,
    pub_sub: P,
    shutdown_notify: Arc<Notify>,
    query_runner: Q,
    node_public_key: NodePublicKey,
    reconfigure_notify: Arc<Notify>,
    notifier: NE,
    event_tx_rx: oneshot::Receiver<Events>,
) {
    info!("Waiting for event sender in execution worker.");
    let event_tx = event_tx_rx.await.expect("Failed to receive event sender");
    info!("Received event sender in execution worker.");

    info!("Execution node messageworker is running");
    let committee = query_runner.get_committee_members_by_index();
    let quorom_threshold = (committee.len() * 2) / 3 + 1;
    let our_index = query_runner
        .pubkey_to_index(&node_public_key)
        .unwrap_or(u32::MAX);
    let on_committee = committee.contains(&our_index);
    let (timeout_tx, mut timeout_rx) = mpsc::channel(128);
    // `pending_timeouts` is not a cache because we already limit the number of timeouts we spawn
    // with `MAX_PENDING_TIMEOUTS`, so `pending_timeouts` is bounded from above by that constant
    let pending_timeouts = HashSet::new();
    let pending_requests = Cache::new(100);

    let mut ctx = Context {
        executor,
        txn_store: TransactionStore::default(),
        pending_digests: HashSet::with_capacity(100),
        executed_digests: HashSet::with_capacity(100),
        quorom_threshold,
        committee,
        our_index,
        on_committee,
        node_public_key,
        pending_timeouts,
        pending_requests,
        query_runner,
        pub_sub,
        event_tx,
        notifier,
        timeout_tx,
        reconfigure_notify,
        last_executed_timestamp: None,
        estimated_tbe: Duration::from_secs(30),
        deviation_tbe: Duration::from_secs(5),
    };

    let shutdown_future = shutdown_notify.notified();
    pin!(shutdown_future);
    loop {
        // todo(dalton): revisit pinning these and using Notify over oneshot

        tokio::select! {
            biased;
            _ = &mut shutdown_future => {
                ctx.executor.downgrade();
                break;
            },
            output = consensus_output_rx.recv() => {
                if !ctx.on_committee {
                    // This should never happen if it somehow does there is critical error somewhere
                    panic!("We received consensus output from narwhal while not on the committee");
                }
                // This branch is only executed by validators.
                let Some(consensus_output) = output else {
                    break;
                };
                handle_consensus_output(&mut ctx, consensus_output).await;
            }
            Some(msg) = ctx.pub_sub.recv_event() => {
                handle_pubsub_event::<P, Q, NE>(msg, &mut ctx).await;
            },
            digest = timeout_rx.recv() => {
                // Timeout for a missing parcel. If we still haven't received the parcel, we send a
                // request.
                if let Some(digest) = digest {
                    ctx.pending_timeouts.remove(&digest);

                    if ctx.txn_store.get_parcel(&digest).is_none() {
                        let request = PubSubMsg::RequestTransactions(digest);
                        let _ = ctx.pub_sub.send(&request, None).await;
                        ctx.pending_requests.insert(digest, ());
                        info!("Send request for missing parcel with digest: {digest:?}");

                        increment_counter!(
                            "consensus_missing_parcel_request",
                            Some("Counter for the number of times the node sent a request for a missing consensus parcel")
                        );
                    }
                }
            }
        }
    }
}

async fn handle_consensus_output<P: PubSub<PubSubMsg>, Q: SyncQueryRunnerInterface, NE: Emitter>(
    ctx: &mut Context<P, Q, NE>,
    consensus_output: ConsensusOutput,
) {
    let current_epoch = ctx.query_runner.get_current_epoch();
    let sub_dag_index = consensus_output.sub_dag.sub_dag_index;

    let batch_payload: Vec<Vec<u8>> = consensus_output
        .sub_dag
        .certificates
        .iter()
        .zip(consensus_output.batches.into_iter())
        .filter_map(|(cert, batch)| {
            // Skip over the ones that have a different epoch. Shouldnt ever happen besides an
            // edge case towards the end of an epoch
            if cert.epoch() != current_epoch {
                error!("we recieved a consensus cert from an epoch we are not on");
                None
            } else {
                // Map the batch to just the transactions
                Some(
                    batch
                        .into_iter()
                        .flat_map(|batch| match batch {
                            // work around because batch.transactions() would require clone
                            Batch::V1(btch) => btch.transactions,
                            Batch::V2(btch) => btch.transactions,
                        })
                        .collect::<Vec<Vec<u8>>>(),
                )
            }
        })
        .flatten()
        .collect();

    if batch_payload.is_empty() {
        return;
    }
    // We have batches in the payload send them over broadcast along with an attestion
    // of them
    let last_executed = ctx.query_runner.get_last_block();
    let parcel = AuthenticStampedParcel {
        transactions: batch_payload.clone(),
        last_executed,
        epoch: current_epoch,
        sub_dag_index,
    };

    let epoch_changed = submit_batch(ctx, batch_payload, parcel.to_digest(), sub_dag_index).await;

    let parcel_digest = parcel.to_digest();
    let attestation = CommitteeAttestation {
        digest: parcel_digest,
        node_index: ctx.our_index,
        epoch: parcel.epoch,
    };

    info!("Send transaction parcel to broadcast as a validator");
    let _ = ctx.pub_sub.send(&attestation.into(), None).await;
    let msg_digest = ctx.pub_sub.send(&parcel.clone().into(), None).await;

    ctx.txn_store
        .store_parcel(parcel, ctx.our_index, msg_digest.ok());
    // No need to store the attestation we have already executed it

    if epoch_changed {
        ctx.reconfigure_notify.notify_waiters();

        ctx.committee = ctx.query_runner.get_committee_members_by_index();
        ctx.quorom_threshold = (ctx.committee.len() * 2) / 3 + 1;
        // We recheck our index incase it was non existant before
        // and we staked during this epoch and finally got the certificate
        ctx.our_index = ctx
            .query_runner
            .pubkey_to_index(&ctx.node_public_key)
            .unwrap_or(u32::MAX);
        ctx.on_committee = ctx.committee.contains(&ctx.our_index);

        ctx.txn_store.change_epoch(&ctx.committee);
    }
}

// Returns true if the epoch changed
async fn submit_batch<P: PubSub<PubSubMsg>, Q: SyncQueryRunnerInterface, NE: Emitter>(
    ctx: &Context<P, Q, NE>,
    payload: Vec<Transaction>,
    digest: Digest,
    sub_dag_index: u64,
) -> bool {
    let transactions = payload
        .into_iter()
        .filter_map(|txn| {
            // Filter out transactions that wont serialize or have already been executed
            if let Ok(txn) = TransactionRequest::try_from(txn.as_ref()) {
                if !ctx.query_runner.has_executed_digest(txn.hash()) {
                    Some(txn)
                } else {
                    None
                }
            } else {
                None
            }
        })
        .collect::<Vec<_>>();

    let block = Block {
        digest,
        sub_dag_index,
        transactions,
    };

    let archive_block = block.clone();

    // Unfailable
    let response = ctx.executor.run(block).await.unwrap();
    info!("Consensus submitted new block to application");

    ctx.event_tx.send(
        response
            .txn_receipts
            .iter()
            .filter_map(|r| r.event.clone())
            .collect(),
    );

    let change_epoch = response.change_epoch;
    ctx.notifier.new_block(archive_block, response);

    if change_epoch {
        let epoch_number = ctx.query_runner.get_current_epoch();
        let epoch_hash = ctx
            .query_runner
            .get_metadata(&Metadata::LastEpochHash)
            .expect("We should have a last epoch hash")
            .maybe_hash()
            .expect("We should have gotten a hash, this is a bug");

        ctx.notifier.epoch_changed(epoch_number, epoch_hash);
    }

    change_epoch
}

async fn handle_pubsub_event<P: PubSub<PubSubMsg>, Q: SyncQueryRunnerInterface, NE: Emitter>(
    mut msg: P::Event,
    ctx: &mut Context<P, Q, NE>,
) {
    match msg.take().unwrap() {
        PubSubMsg::Transactions(parcel) => {
            handle_parcel::<P, Q, NE>(msg, parcel, ctx).await;
        },
        PubSubMsg::Attestation(att) => {
            handle_attestation::<P, Q, NE>(msg, att, ctx).await;
        },
        PubSubMsg::RequestTransactions(digest) => {
            if let Some(msg_digest) = ctx
                .txn_store
                .get_parcel(&digest)
                .and_then(|parcel| parcel.message_digest)
            {
                let filter = HashSet::from([msg.originator()]);
                ctx.pub_sub.repropagate(msg_digest, Some(filter)).await;
                info!("Responded to request for missing parcel with digest: {digest:?}");
                increment_counter!(
                    "consensus_missing_parcel_sent",
                    Some("Number of missing parcels served to other nodes"),
                );
            } else {
                increment_counter!(
                    "consensus_missing_parcel_ignored",
                    Some(
                        "Number of parcel requests that were ignored due to not finding it in the transaction store"
                    ),
                );
            }
        },
    }
}

async fn handle_parcel<P: PubSub<PubSubMsg>, Q: SyncQueryRunnerInterface, NE: Emitter>(
    msg: P::Event,
    parcel: AuthenticStampedParcel,
    ctx: &mut Context<P, Q, NE>,
) {
    let epoch = ctx.query_runner.get_current_epoch();
    let originator = msg.originator();
    let is_committee = ctx.committee.contains(&originator);
    if !is_valid_message(is_committee, parcel.epoch, epoch) {
        msg.mark_invalid_sender();
        return;
    }

    let msg_digest = msg.get_digest();
    let parcel_digest = parcel.to_digest();
    let from_next_epoch = parcel.epoch == epoch + 1;
    let last_executed = parcel.last_executed;

    let mut event = None;
    let parcel_request = ctx.pending_requests.remove(&parcel_digest);
    if parcel_request.is_none() && !from_next_epoch {
        // We only want to propagate parcels that we did not request and that
        // are not from the next epoch.
        msg.propagate();
    } else {
        event = Some(msg);
    }

    if from_next_epoch {
        // if the parcel is from the next epoch, we optimistically
        // store it and check if it's from a validator once we change epochs.

        // Note: this unwrap is safe. If `from_next_epoch=true`, then `event`
        // will always be `Some`. Unfortunately, the borrow checker cannot
        // figure this out on its own if we use `msg` directly here.

        ctx.txn_store
            .store_pending_parcel(parcel, originator, Some(msg_digest), event.unwrap());
    } else {
        // only propagate normal parcels, not parcels we requested, and not
        // parcels that we optimistically accept from the next epoch
        ctx.txn_store
            .store_parcel(parcel, originator, Some(msg_digest));
    };

    // Check if we requested this parcel
    if parcel_request.is_some() {
        // This is a parcel that we specifically requested, so
        // we have to set a timeout for the previous parcel, because
        // we swallow the Err return in the loop of `try_execute`.
        if last_executed != [0; 32] {
            set_parcel_timer(
                last_executed,
                get_timeout(ctx),
                ctx.timeout_tx.clone(),
                &mut ctx.pending_timeouts,
            );
        }

        info!("Received requested parcel with digest: {parcel_digest:?}");

        increment_counter!(
            "consensus_missing_parcel_received",
            Some("Number of missing parcels successfully received from other nodes")
        );
    }

    if !ctx.on_committee {
        // If the node is not on the committee, storing parcels is crucial for the consensus.
        // If storing parcels fails for some reason, we want to panic.
        info!("Received transaction parcel from gossip as an edge node");
        execute_digest::<P, Q, NE>(parcel_digest, ctx).await;
    }
}

async fn handle_attestation<P: PubSub<PubSubMsg>, Q: SyncQueryRunnerInterface, NE: Emitter>(
    msg: P::Event,
    att: CommitteeAttestation,
    ctx: &mut Context<P, Q, NE>,
) {
    let originator = msg.originator();

    let epoch = ctx.query_runner.get_current_epoch();
    let is_committee = ctx.committee.contains(&originator);
    if originator != att.node_index || !is_valid_message(is_committee, att.epoch, epoch) {
        msg.mark_invalid_sender();
        return;
    }

    let from_next_epoch = att.epoch == epoch + 1;
    let mut event = None;
    if !from_next_epoch {
        msg.propagate();
    } else {
        event = Some(msg);
    }

    if !ctx.on_committee {
        info!("Received parcel attestation from gossip as an edge node");

        if from_next_epoch {
            // if the attestation is from the next epoch, we optimistically
            // store it and check if it's from a validator once we change epochs
            // Note: this unwrap is safe. If `from_next_epoch=true`, then `event`
            // will always be `Some`. Unfortunately, the borrow checker cannot
            // figure this out on its own if we use `msg` directly here.
            ctx.txn_store
                .store_pending_attestation(att.digest, att.node_index, event.unwrap());
        } else {
            ctx.txn_store.store_attestation(att.digest, att.node_index);
        }

        execute_digest::<P, Q, NE>(att.digest, ctx).await;
    }
}

async fn execute_digest<P: PubSub<PubSubMsg>, Q: SyncQueryRunnerInterface, NE: Emitter>(
    digest: Digest,
    ctx: &mut Context<P, Q, NE>,
) {
    match try_execute(digest, ctx).await {
        Ok(epoch_changed) => {
            if epoch_changed {
                ctx.committee = ctx.query_runner.get_committee_members_by_index();
                ctx.quorom_threshold = (ctx.committee.len() * 2) / 3 + 1;
                // We recheck our index incase it was non existant before and
                // we staked during this epoch and finally got the certificate
                ctx.our_index = ctx
                    .query_runner
                    .pubkey_to_index(&ctx.node_public_key)
                    .unwrap_or(u32::MAX);
                ctx.on_committee = ctx.committee.contains(&ctx.our_index);
                ctx.reconfigure_notify.notify_waiters();
                ctx.txn_store.change_epoch(&ctx.committee);
            }
        },
        Err(not_executed) => {
            handle_not_executed(not_executed, ctx);
        },
    }
}

// Threshold should be 2f + 1 of the committee
// Returns true if the epoch has changed
async fn try_execute<P: PubSub<PubSubMsg>, Q: SyncQueryRunnerInterface, NE: Emitter>(
    digest: Digest,
    ctx: &mut Context<P, Q, NE>,
) -> Result<bool, NotExecuted> {
    // get the current chain head
    let head = ctx.query_runner.get_last_block();
    let mut epoch_changed = match try_execute_internal(digest, head, ctx).await {
        Ok(epoch_changed) => epoch_changed,
        Err(NotExecuted::MissingAttestations(_)) => false,
        Err(e) => return Err(e),
    };

    let digests: Vec<Digest> = ctx.pending_digests.iter().copied().collect();
    for digest in digests {
        if ctx.pending_digests.contains(&digest) {
            // get the current chain head
            let head = ctx.query_runner.get_last_block();
            if let Ok(epoch_changed_) = try_execute_internal(digest, head, ctx).await {
                epoch_changed = epoch_changed || epoch_changed_;
            }
        }
    }
    Ok(epoch_changed)
}

async fn try_execute_internal<P: PubSub<PubSubMsg>, Q: SyncQueryRunnerInterface, NE: Emitter>(
    digest: Digest,
    head: Digest,
    ctx: &mut Context<P, Q, NE>,
) -> Result<bool, NotExecuted> {
    if ctx.executed_digests.contains(&digest) {
        // we already executed this parcel
        return Ok(false);
    } else if let Some(x) = ctx.txn_store.get_attestations(&digest) {
        // we need a quorum of attestations in order to execute the parcel
        if x.len() >= ctx.quorom_threshold {
            // if we should execute we need to make sure we can connect this to our transaction
            // chain
            return try_execute_chain(digest, head, ctx).await;
        }
    }
    Err(NotExecuted::MissingAttestations(digest))
}

async fn try_execute_chain<P: PubSub<PubSubMsg>, Q: SyncQueryRunnerInterface, NE: Emitter>(
    digest: Digest,
    head: Digest,
    ctx: &mut Context<P, Q, NE>,
) -> Result<bool, NotExecuted> {
    let mut txn_chain = VecDeque::new();
    let mut current_digest = digest;
    let mut parcel_chain = Vec::new();

    loop {
        let last_executed;
        if let Some(parcel) = ctx.txn_store.get_parcel(&current_digest) {
            if parcel.inner.epoch < ctx.query_runner.get_current_epoch()
                || (parcel.inner.last_executed == [0; 32] && head != [0; 32])
            {
                // if the parcel is from the previous epoch, or it points to the zero digest,
                // even though the head isn't the zero digest, we abort and throw away
                // the current parcel chain
                return Err(NotExecuted::RejectedParcel(current_digest));
            }
            // if parcel.inner.last_executed == [0; 32] && head == [0; 32], then the check
            // `last_executed == head` further down will be true and we will execute.
            parcel_chain.push(current_digest);
            txn_chain.push_front((
                parcel.inner.transactions.clone(),
                parcel.inner.sub_dag_index,
                current_digest,
            ));
            last_executed = parcel.inner.last_executed;
        } else if current_digest != [0; 32] {
            // parcel not available, we will set a timer for a missing parcel request
            for digest in parcel_chain {
                ctx.pending_digests.insert(digest);
            }
            return Err(NotExecuted::MissingParcel(current_digest));
        } else {
            // This case cannot happen. `current_digest` cannot be [0; 32]. If `current_digest`
            // was [0; 32], then `last_executed` was [0; 32] in the previous
            // iteration of the loop. If `last_executed` was [0; 32], then there
            // are two cases:
            // 1. `head` is [0; 32]. In this case the check `last_executed == head` is true, so we
            //    will execute the parcel chain and return without going to the next loop iteration.
            // 2. `head` is not [0; 32]. In this case we abort and return without going to the next
            //    loop iteration.
            // In both cases we don't go to the next loop iteration, so `current_digest` will
            // never be set to [0; 32].
            // We only handle this case to appease the compiler. Otherwise `last_executed`
            // could be uninitialized.
            return Err(NotExecuted::RejectedParcel(current_digest));
        }

        if last_executed == head {
            let mut epoch_changed = false;

            // We connected the chain now execute all the transactions
            for (batch, sub_dag_index, digest) in txn_chain {
                if submit_batch(ctx, batch, digest, sub_dag_index).await {
                    epoch_changed = true;
                }
            }

            // mark all parcels in chain as executed
            for digest in parcel_chain {
                ctx.pending_digests.remove(&digest);
                ctx.executed_digests.insert(digest);
            }

            // TODO(matthias): technically this call should be inside the for loop where we
            // call `submit_batch`, but I think this might bias the estimate to be too low.
            update_estimated_tbe(ctx);

            return Ok(epoch_changed);
        } else {
            current_digest = last_executed;
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
fn handle_not_executed<P: PubSub<PubSubMsg>, Q: SyncQueryRunnerInterface, NE: Emitter>(
    not_executed: NotExecuted,
    ctx: &mut Context<P, Q, NE>,
) {
    if let NotExecuted::MissingParcel(digest) = not_executed {
        set_parcel_timer(
            digest,
            get_timeout(ctx),
            ctx.timeout_tx.clone(),
            &mut ctx.pending_timeouts,
        );
    }
}

fn set_parcel_timer(
    digest: Digest,
    timeout: Duration,
    timeout_tx: mpsc::Sender<Digest>,
    pending_timeouts: &mut HashSet<Digest>,
) {
    if !pending_timeouts.contains(&digest) && pending_timeouts.len() < MAX_PENDING_TIMEOUTS {
        spawn!(
            async move {
                tokio::time::sleep(timeout).await;
                let _ = timeout_tx.send(digest).await;
            },
            "CONSENSUS: parcel timer"
        );
        pending_timeouts.insert(digest);
    }
}

fn update_estimated_tbe<P: PubSub<PubSubMsg>, Q: SyncQueryRunnerInterface, NE: Emitter>(
    ctx: &mut Context<P, Q, NE>,
) {
    if let Some(timestamp) = ctx.last_executed_timestamp {
        if let Ok(sample_tbe) = timestamp.elapsed() {
            let sample_tbe = sample_tbe.as_millis() as f64;
            let estimated_tbe = ctx.estimated_tbe.as_millis() as f64;
            let new_estimated_tbe = (1.0 - TBE_EMA) * estimated_tbe + TBE_EMA * sample_tbe;
            ctx.estimated_tbe = Duration::from_millis(new_estimated_tbe as u64);

            let deviation_tbe = ctx.deviation_tbe.as_millis() as f64;
            let new_deviation_tbe =
                (1.0 - TBE_EMA) * deviation_tbe + TBE_EMA * (new_estimated_tbe - sample_tbe).abs();
            ctx.deviation_tbe = Duration::from_millis(new_deviation_tbe as u64);
        }
    }
    ctx.last_executed_timestamp = Some(SystemTime::now());
}

fn get_timeout<P: PubSub<PubSubMsg>, Q: SyncQueryRunnerInterface, NE: Emitter>(
    ctx: &Context<P, Q, NE>,
) -> Duration {
    let mut timeout = 4 * ctx.deviation_tbe + ctx.estimated_tbe;
    timeout = timeout.max(MIN_TBE);
    timeout = timeout.min(MAX_TBE);
    timeout
}

// The parcel must be either from the current committee or from the
// next epoch. We optimistically accept parcels from the following
// epoch in case we missed the epoch change parcel. We will verify
// later on that the parcel was sent from a committee member.
fn is_valid_message(in_committee: bool, msg_epoch: Epoch, current_epoch: Epoch) -> bool {
    (in_committee && msg_epoch == current_epoch) || msg_epoch == current_epoch + 1
}

#[derive(Debug)]
pub enum NotExecuted {
    MissingParcel(Digest),
    RejectedParcel(Digest),
    MissingAttestations(Digest),
}

#[cfg(test)]
mod tests {
    use crate::execution::worker::is_valid_message;

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
