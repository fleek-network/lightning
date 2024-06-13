use std::sync::Arc;
use std::time::Duration;

use fleek_crypto::NodePublicKey;
use lightning_interfaces::types::{Digest as BroadcastDigest, NodeIndex};
use lightning_interfaces::{
    spawn,
    BroadcastEventInterface,
    Emitter,
    PubSub,
    SyncQueryRunnerInterface,
    ToDigest,
};
use lightning_utils::application::QueryRunnerExt;
use tokio::pin;
use tokio::sync::{mpsc, oneshot, Notify};
use tokio::task::JoinHandle;
use tracing::{error, info};

use crate::consensus::PubSubMsg;
use crate::execution::{AuthenticStampedParcel, CommitteeAttestation, Digest, Execution};

pub(crate) mod ring_buffer;
mod transaction_store;
use transaction_store::TransactionStore;

pub use self::transaction_store::{NotExecuted, Parcel};

pub enum TxnStoreCmd<T: BroadcastEventInterface<PubSubMsg>> {
    StoreParcel {
        parcel: AuthenticStampedParcel,
        originator: NodeIndex,
        message_digest: Option<BroadcastDigest>,
    },
    StorePendingParcel {
        parcel: AuthenticStampedParcel,
        originator: NodeIndex,
        message_digest: BroadcastDigest,
        event: T,
    },
    StoreAttestation {
        digest: Digest,
        node_index: NodeIndex,
    },
    StorePendingAttestation {
        digest: Digest,
        node_index: NodeIndex,
        event: T,
    },
    GetParcelMessageDigest {
        digest: Digest,
        response: oneshot::Sender<Option<BroadcastDigest>>,
    },
    ContainsParcel {
        digest: Digest,
        response: oneshot::Sender<bool>,
    },
    TryExecute {
        digest: Digest,
        quorom_threshold: usize,
        response: oneshot::Sender<Result<bool, NotExecuted>>,
    },
    GetTimeout {
        response: oneshot::Sender<Duration>,
    },
}

pub struct TransactionStoreManager {
    handle: JoinHandle<()>,
    tx_shutdown: Arc<Notify>,
}

struct Context<P: PubSub<PubSubMsg>, Q: SyncQueryRunnerInterface, NE: Emitter> {
    our_index: NodeIndex,
    on_committee: bool,
    committee: Vec<NodeIndex>,
    node_public_key: NodePublicKey,
    pub_sub: P,
    query_runner: Q,
    execution: Arc<Execution<Q, NE>>,
    txn_store: TransactionStore<P::Event>,
}

impl TransactionStoreManager {
    pub fn spawn<P: PubSub<PubSubMsg> + 'static, Q: SyncQueryRunnerInterface, NE: Emitter>(
        cmd_ŕx: mpsc::Receiver<TxnStoreCmd<P::Event>>,
        execution: Arc<Execution<Q, NE>>,
        query_runner: Q,
        rx_narwhal_batch: mpsc::Receiver<(AuthenticStampedParcel, bool)>,
        pub_sub: P,
        node_public_key: NodePublicKey,
    ) -> Self {
        let shutdown_notify = Arc::new(Notify::new());

        let handle = spawn!(
            spawn_txn_worker::<P, Q, NE>(
                cmd_ŕx,
                execution,
                query_runner,
                rx_narwhal_batch,
                pub_sub,
                node_public_key,
                shutdown_notify.clone(),
            ),
            "CONSENSUS: transaction store worker"
        );

        Self {
            handle,
            tx_shutdown: shutdown_notify,
        }
    }

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

async fn spawn_txn_worker<
    P: PubSub<PubSubMsg> + 'static,
    Q: SyncQueryRunnerInterface,
    NE: Emitter,
>(
    mut cmd_ŕx: mpsc::Receiver<TxnStoreCmd<P::Event>>,
    execution: Arc<Execution<Q, NE>>,
    query_runner: Q,
    mut rx_narwhal_batch: mpsc::Receiver<(AuthenticStampedParcel, bool)>,
    pub_sub: P,
    node_public_key: NodePublicKey,
    shutdown_notify: Arc<Notify>,
) {
    let txn_store = TransactionStore::<P::Event>::new();
    let our_index = query_runner
        .pubkey_to_index(&node_public_key)
        .unwrap_or(u32::MAX);
    let committee = query_runner.get_committee_members_by_index();
    let on_committee = committee.contains(&our_index);

    let mut ctx = Context {
        our_index,
        on_committee,
        committee,
        node_public_key,
        pub_sub,
        query_runner,
        execution,
        txn_store,
    };

    let shutdown_future = shutdown_notify.notified();
    pin!(shutdown_future);
    loop {
        tokio::select! {
            _ = &mut shutdown_future => {
                break;
            },
            cmd = cmd_ŕx.recv() => {
                let Some(cmd) = cmd else {
                    break;
                };
                handle_cmd::<P, Q, NE>(cmd, &mut ctx).await;
            }
            Some((parcel, epoch_changed)) = rx_narwhal_batch.recv() => {
                if !on_committee {
                    // This should never happen if it somehow does there is critical error somewhere
                    panic!("We somehow sent ourselves a parcel from narwhal while not on committee");
                }
                handle_batch(parcel, epoch_changed, &mut ctx).await;
            },
        }
    }
}

async fn handle_batch<P: PubSub<PubSubMsg>, Q: SyncQueryRunnerInterface, NE: Emitter>(
    parcel: AuthenticStampedParcel,
    epoch_changed: bool,
    ctx: &mut Context<P, Q, NE>,
) {
    // This will only be executed by validator nodes
    let parcel_digest = parcel.to_digest();
    let attestation = CommitteeAttestation {
        digest: parcel_digest,
        node_index: ctx.our_index,
        epoch: parcel.epoch,
    };

    info!("Send transaction parcel to broadcast as a validator");
    let _ = ctx.pub_sub.send(&attestation.into(), None).await;

    if let Ok(msg_digest) = ctx.pub_sub.send(&parcel.clone().into(), None).await {
        ctx.txn_store
            .store_parcel(parcel, ctx.our_index, Some(msg_digest));
    } else {
        ctx.txn_store.store_parcel(parcel, ctx.our_index, None);
    }
    // No need to store the attestation we have already executed it

    if epoch_changed {
        ctx.committee = ctx.query_runner.get_committee_members_by_index();
        //quorom_threshold = (committee.len() * 2) / 3 + 1;
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

async fn handle_cmd<P: PubSub<PubSubMsg>, Q: SyncQueryRunnerInterface, NE: Emitter>(
    cmd: TxnStoreCmd<P::Event>,
    ctx: &mut Context<P, Q, NE>,
) {
    match cmd {
        TxnStoreCmd::StoreParcel {
            parcel,
            originator,
            message_digest,
        } => {
            ctx.txn_store
                .store_parcel(parcel, originator, message_digest);
        },
        TxnStoreCmd::StorePendingParcel {
            parcel,
            originator,
            message_digest,
            event,
        } => {
            ctx.txn_store
                .store_pending_parcel(parcel, originator, message_digest, event);
        },
        TxnStoreCmd::StoreAttestation { digest, node_index } => {
            ctx.txn_store.store_attestation(digest, node_index);
        },
        TxnStoreCmd::StorePendingAttestation {
            digest,
            node_index,
            event,
        } => {
            ctx.txn_store
                .store_pending_attestation(digest, node_index, event);
        },
        TxnStoreCmd::GetParcelMessageDigest { digest, response } => {
            let parcel = ctx.txn_store.get_parcel(&digest);
            if let Err(e) = response.send(parcel.and_then(|p| p.message_digest)) {
                error!("Failed to respond to get parcel msg digest command in txn manager: {e:?}");
            }
        },
        TxnStoreCmd::ContainsParcel { digest, response } => {
            let parcel = ctx.txn_store.get_parcel(&digest);
            if let Err(e) = response.send(parcel.is_some()) {
                error!("Failed to respond to get contains parcel command in txn manager: {e:?}");
            }
        },
        TxnStoreCmd::TryExecute {
            digest,
            quorom_threshold,
            response,
        } => {
            // This will only be executed by edge nodes
            let res = ctx
                .txn_store
                .try_execute(digest, quorom_threshold, &ctx.query_runner, &ctx.execution)
                .await;

            if let Ok(true) = &res {
                let committee = ctx.query_runner.get_committee_members_by_index();
                ctx.txn_store.change_epoch(&committee);
            }

            if let Err(e) = response.send(res) {
                error!("Failed to respond to try execute command in txn manager: {e:?}");
            }
        },
        TxnStoreCmd::GetTimeout { response } => {
            let timeout = ctx.txn_store.get_timeout();

            if let Err(e) = response.send(timeout) {
                error!("Failed to respond to get timeout command in txn manager: {e:?}");
            }
        },
    }
}
