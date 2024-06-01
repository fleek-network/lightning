#[cfg(test)]
pub mod tests;

use std::collections::VecDeque;
use std::marker::PhantomData;
use std::sync::Arc;
use std::time::{Duration, SystemTime};

use affair::{AsyncWorker, Socket};
use fleek_crypto::{NodePublicKey, NodeSecretKey, SecretKey, TransactionSender};
use lightning_interfaces::prelude::*;
use lightning_interfaces::types::{
    NodeIndex,
    TransactionResponse,
    UpdateMethod,
    UpdatePayload,
    UpdateRequest,
};
use lightning_interfaces::BlockExecutedNotification;
use lightning_utils::application::QueryRunnerExt;
use tokio::sync::Mutex;
use tracing::error;

// If a transaction does not get ordered, the signer will try to resend it.
// `TIMEOUT` specifies the duration the signer will wait before resending transactions to the
// mempool.
// In mainnet, this should be less than 12 secs.
#[cfg(not(test))]
const TIMEOUT: Duration = Duration::from_secs(60);
#[cfg(test)]
const TIMEOUT: Duration = Duration::from_secs(3);

// Maximum number of times we will resend a transaction.
const MAX_RETRIES: u8 = 3;

pub struct Signer<C: Collection> {
    socket: Socket<UpdateMethod, u64>,
    worker: SignerWorker,
    _c: PhantomData<C>,
}

#[derive(Clone)]
struct SignerWorker {
    state: Arc<Mutex<SignerState>>,
}

struct SignerState {
    node_secret_key: NodeSecretKey,
    node_public_key: NodePublicKey,
    mempool_socket: MempoolSocket,
    chain_id: Option<u32>,
    base_nonce: u64,
    next_nonce: u64,
    base_timestamp: Option<SystemTime>,
    pending_transactions: VecDeque<PendingTransaction>,
}

struct LazyNodeIndex {
    node_public_key: NodePublicKey,
    node_index: Option<NodeIndex>,
}

impl<C: Collection> Signer<C> {
    pub fn init(keystore: &C::KeystoreInterface, forwarder: &C::ForwarderInterface) -> Self {
        let state = SignerState {
            node_secret_key: keystore.get_ed25519_sk(),
            node_public_key: keystore.get_ed25519_pk(),
            mempool_socket: forwarder.mempool_socket(),
            chain_id: None,
            base_nonce: 0,
            next_nonce: 0,
            base_timestamp: None,
            pending_transactions: VecDeque::new(),
        };

        let worker = SignerWorker {
            state: Arc::new(Mutex::new(state)),
        };

        let socket = worker.clone().spawn();

        Self {
            socket,
            worker,
            _c: PhantomData,
        }
    }

    pub async fn start(
        this: fdi::Ref<Self>,
        notifier: fdi::Ref<C::NotifierInterface>,
        fdi::Cloned(query_runner): fdi::Cloned<c![C::ApplicationInterface::SyncExecutor]>,
    ) {
        let subscriber = notifier.subscribe_block_executed();
        let worker = this.worker.clone();

        // Initialize the worker's state.
        let mut guard = worker.state.lock().await;
        let mut node_index = LazyNodeIndex::new(guard.node_public_key);
        let chain_id = query_runner.get_chain_id();
        let nonce = node_index.query_nonce(&query_runner);
        guard.init_state(chain_id, nonce);
        drop(guard);

        spawn!(
            async move {
                new_block_task(node_index, worker, subscriber, query_runner).await;
            },
            "SIGNER: new block task"
        );
    }
}

impl<C: Collection> SignerInterface<C> for Signer<C> {
    /// Returns a socket that can be used to submit transactions to the mempool, these
    /// transactions are signed by the node and a proper nonce is assigned by the
    /// implementation.
    ///
    /// # Panics
    ///
    /// This function can panic if there has not been a prior call to `provide_mempool`.
    fn get_socket(&self) -> SubmitTxSocket {
        self.socket.clone()
    }
}

impl SignerState {
    fn init_state(&mut self, chain_id: u32, base_nonce: u64) {
        self.base_nonce = base_nonce;
        self.next_nonce = base_nonce + 1;
        self.chain_id = Some(chain_id);
    }

    async fn sign_new_tx(&mut self, method: UpdateMethod) -> u64 {
        let assigned_nonce = self.next_nonce;
        let update_payload = UpdatePayload {
            sender: TransactionSender::NodeMain(self.node_public_key),
            method,
            nonce: assigned_nonce,
            chain_id: self.chain_id.unwrap(),
        };

        let digest = update_payload.to_digest();
        let signature = self.node_secret_key.sign(&digest);
        let update_request = UpdateRequest {
            signature: signature.into(),
            payload: update_payload,
        };

        if let Err(e) = self
            .mempool_socket
            .enqueue(update_request.clone().into())
            .await
            .map_err(|r| anyhow::anyhow!(format!("{r:?}")))
        {
            error!("Failed to send transaction to mempool: {e:?}");
        }

        // Optimistically increment nonce
        self.next_nonce += 1;

        let timestamp = SystemTime::now();
        self.pending_transactions.push_back(PendingTransaction {
            update_request,
            timestamp,
            tries: 1,
        });

        // Set timer
        if self.base_timestamp.is_none() {
            self.base_timestamp = Some(timestamp);
        }

        assigned_nonce
    }

    async fn sync_with_application<Q>(&mut self, application_nonce: u64, query_runner: &Q)
    where
        Q: SyncQueryRunnerInterface,
    {
        // All transactions in range [base_nonce, application_nonce] have
        // been ordered, so we can remove them from `pending_transactions`.
        self.base_nonce = application_nonce;

        while !self.pending_transactions.is_empty()
            && self.pending_transactions[0].update_request.payload.nonce <= application_nonce
        {
            self.pending_transactions.pop_front();
        }

        if self.pending_transactions.is_empty() {
            self.base_timestamp = None;
        } else if let Some(base_timestamp) = self.base_timestamp {
            if base_timestamp.elapsed().unwrap() >= TIMEOUT {
                // At this point we assume that the transactions in the buffer will never get
                // ordered.
                self.base_timestamp = None;
                // Reset `next_nonce` to the nonce the application is expecting.
                self.next_nonce = self.base_nonce + 1;
                // Resend all transactions in the buffer.

                for tx in self.pending_transactions.iter_mut() {
                    if matches!(
                        query_runner.simulate_txn(tx.update_request.clone().into()),
                        TransactionResponse::Revert(_)
                    ) || tx.tries >= MAX_RETRIES
                    {
                        // If transaction reverts or we reached the maximum number of retries, don't
                        // retry again.
                        // To prevent invalidating the nonces of the following pending transactions,
                        // we have to increment the nonce on the application state.
                        let method = UpdateMethod::IncrementNonce {};
                        let update_payload = UpdatePayload {
                            sender: TransactionSender::NodeMain(self.node_public_key),
                            method,
                            nonce: self.next_nonce,
                            chain_id: self.chain_id.unwrap(),
                        };
                        let digest = update_payload.to_digest();
                        let signature = self.node_secret_key.sign(&digest);
                        let update_request = UpdateRequest {
                            signature: signature.into(),
                            payload: update_payload,
                        };
                        tx.update_request = update_request;
                    } else {
                        // Since we just replace transactions that we don't resend with an
                        // increment nonce transaction, we don't have to update the nonce of the
                        // transactions we try to resend.
                        assert_eq!(tx.update_request.payload.nonce, self.next_nonce);
                    }
                    // Update timestamp to resending time.
                    tx.timestamp = SystemTime::now();
                    if self.base_timestamp.is_none() {
                        self.base_timestamp = Some(tx.timestamp);
                    }

                    self.next_nonce += 1;
                }

                for pending_tx in self.pending_transactions.iter_mut() {
                    if let Err(e) = self
                        .mempool_socket
                        .run(pending_tx.update_request.clone().into())
                        .await
                        .map_err(|r| anyhow::anyhow!(format!("{r:?}")))
                    {
                        error!("Failed to send transaction to mempool: {e:?}");
                    } else {
                        pending_tx.tries += 1;
                    }
                }
            }
        }
    }
}

impl LazyNodeIndex {
    fn new(node_public_key: NodePublicKey) -> Self {
        Self {
            node_public_key,
            node_index: None,
        }
    }

    /// Query the application layer for the last nonce and returns it.
    fn query_nonce<Q>(&mut self, query_runner: &Q) -> u64
    where
        Q: SyncQueryRunnerInterface,
    {
        if self.node_index.is_none() {
            self.node_index = query_runner.pubkey_to_index(&self.node_public_key);
        }

        self.node_index
            .and_then(|node_index| query_runner.get_node_info(&node_index, |n| n.nonce))
            .unwrap_or(0)
    }
}

impl AsyncWorker for SignerWorker {
    type Request = UpdateMethod;
    type Response = u64;

    async fn handle(&mut self, method: UpdateMethod) -> u64 {
        let mut state = self.state.lock().await;
        state.sign_new_tx(method).await
    }
}

impl<C: Collection> BuildGraph for Signer<C> {
    fn build_graph() -> fdi::DependencyGraph {
        fdi::DependencyGraph::new().with_infallible(
            Self::init.with_event_handler("start", Self::start.wrap_with_block_on()),
        )
    }
}

#[derive(Clone)]
struct PendingTransaction {
    pub update_request: UpdateRequest,
    pub timestamp: SystemTime,
    pub tries: u8,
}

async fn new_block_task<Q: SyncQueryRunnerInterface>(
    mut node_index: LazyNodeIndex,
    worker: SignerWorker,
    mut subscriber: impl Subscriber<BlockExecutedNotification>,
    query_runner: Q,
) {
    while let Some(_notification) = subscriber.last().await {
        let nonce = node_index.query_nonce(&query_runner);
        // TODO(qti3e): Get the lock only if we have to. Timeout should get sep from block.
        // Right now we are relying on the existence of new blocks to handle timeout.
        let mut guard = worker.state.lock().await;
        guard.sync_with_application(nonce, &query_runner).await;
    }
}
