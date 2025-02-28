use std::collections::VecDeque;
use std::marker::PhantomData;
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime};

use affair::AsyncWorker;
use anyhow::anyhow;
use fleek_crypto::{NodePublicKey, NodeSecretKey, SecretKey, TransactionSender};
use lightning_interfaces::prelude::*;
use lightning_interfaces::types::{
    ExecuteTransaction,
    NodeIndex,
    TransactionReceipt,
    TransactionResponse,
    UpdateMethod,
    UpdatePayload,
    UpdateRequest,
};
use lightning_interfaces::{spawn_worker, BlockExecutedNotification};
use lightning_utils::application::QueryRunnerExt;
use quick_cache::sync::Cache;
use tokio::sync::{oneshot, Mutex};
use tracing::{debug, error, warn};

use crate::listener::BlockListener;

// If a transaction does not get ordered, the signer will try to resend it.
// `TIMEOUT` specifies the duration the signer will wait before resending transactions to the
// mempool.
// In mainnet, this should be less than 12 secs.
#[cfg(not(test))]
const TIMEOUT: Duration = Duration::from_secs(60);
#[cfg(test)]
const TIMEOUT: Duration = Duration::from_secs(3);

// Timeout for awaiting the transaction receipt.
const TXN_RECEIPT_TIMEOUT: Duration = Duration::from_secs(30);
// Interval for checking if the receipt is available.
const TXN_RECEIPT_INTERVAL: Duration = Duration::from_millis(100);

// Maximum number of times we will resend a transaction.
const MAX_RETRIES: u8 = 3;

// Maximum number of times we will resend to the forwarder.
const MAX_FORWARDER_RETRIES: u8 = 3;

// Waiting time between forwarder retries.
const WAIT_FOR_RETRY: Duration = Duration::from_secs(5);

// Receipt cache capacity.
const CACHE_CAPACITY: usize = 1000;

pub struct Signer<C: NodeComponents> {
    socket: SignerSubmitTxSocket,
    worker: SignerWorker<C>,
    _c: PhantomData<C>,
}

#[derive(Clone)]
struct SignerWorker<C: NodeComponents> {
    state: Arc<Mutex<SignerState<C>>>,
}

struct SignerState<C: NodeComponents> {
    query_runner: c![C::ApplicationInterface::SyncExecutor],
    node_secret_key: NodeSecretKey,
    node_public_key: NodePublicKey,
    mempool_socket: MempoolSocket,
    chain_id: Option<u32>,
    base_nonce: u64,
    next_nonce: u64,
    base_timestamp: Option<SystemTime>,
    pending_transactions: VecDeque<PendingTransaction>,
    receipt_cache: Arc<Cache<[u8; 32], TransactionReceipt>>,
}

struct LazyNodeIndex {
    node_public_key: NodePublicKey,
    node_index: Option<NodeIndex>,
}

impl<C: NodeComponents> Signer<C> {
    pub fn init(
        keystore: &C::KeystoreInterface,
        forwarder: &C::ForwarderInterface,
        app: &C::ApplicationInterface,
        fdi::Cloned(notifier): fdi::Cloned<C::NotifierInterface>,
        fdi::Cloned(waiter): fdi::Cloned<lightning_interfaces::ShutdownWaiter>,
    ) -> Self {
        let query_runner = app.sync_query();

        let receipt_cache = Arc::new(Cache::new(CACHE_CAPACITY));
        let listener = BlockListener::<C>::new(receipt_cache.clone(), notifier.clone());

        let state = SignerState {
            query_runner,
            node_secret_key: keystore.get_ed25519_sk(),
            node_public_key: keystore.get_ed25519_pk(),
            mempool_socket: forwarder.mempool_socket(),
            chain_id: None,
            base_nonce: 0,
            next_nonce: 0,
            base_timestamp: None,
            pending_transactions: VecDeque::new(),
            receipt_cache,
        };

        let worker = SignerWorker {
            state: Arc::new(Mutex::new(state)),
        };

        spawn!(
            async move {
                listener.start().await;
            },
            "SIGNER: block listener task"
        );
        let socket = spawn_worker!(worker.clone(), "SIGNER", waiter, crucial);

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
        let nonce = node_index.query_nonce(&query_runner);
        guard.init_state(nonce);
        drop(guard);

        spawn!(
            async move {
                new_block_task(node_index, worker, subscriber, query_runner).await;
            },
            "SIGNER: new block task"
        );

        tracing::debug!("signer started");
    }
}

impl<C: NodeComponents> SignerInterface<C> for Signer<C> {
    /// Returns a socket that can be used to submit transactions to the mempool, these
    /// transactions are signed by the node and a proper nonce is assigned by the
    /// implementation.
    ///
    /// # Panics
    ///
    /// This function can panic if there has not been a prior call to `provide_mempool`.
    fn get_socket(&self) -> SignerSubmitTxSocket {
        self.socket.clone()
    }
}

impl<C: NodeComponents> SignerState<C> {
    fn init_state(&mut self, base_nonce: u64) {
        self.base_nonce = base_nonce;
        self.next_nonce = base_nonce + 1;
    }

    async fn sign_new_tx(&mut self, request: ExecuteTransaction) -> u64 {
        if self.chain_id.is_none() {
            self.chain_id = Some(self.query_runner.get_chain_id());
        }

        let ExecuteTransaction { method, receipt_tx } = request;

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

        if let Err(e) = send_to_forwarder(&self.mempool_socket, &update_request).await {
            error!("failed to send transaction to mempool: {e:?}");
        }

        // Optimistically increment nonce
        self.next_nonce += 1;

        let timestamp = SystemTime::now();
        self.pending_transactions.push_back(PendingTransaction {
            update_request,
            timestamp,
            tries: 1,
            receipt_tx,
        });

        // Set timer
        if self.base_timestamp.is_none() {
            self.base_timestamp = Some(timestamp);
        }

        assigned_nonce
    }

    async fn sync_with_application(&mut self, application_nonce: u64) {
        // All transactions in range [base_nonce, application_nonce] have
        // been ordered, so we can remove them from `pending_transactions`.
        self.base_nonce = application_nonce;
        while !self.pending_transactions.is_empty()
            && self.pending_transactions[0].update_request.payload.nonce <= application_nonce
        {
            if let Some(txn) = self.pending_transactions.pop_front() {
                // Check if the request contained a receipt sender.
                // In this case we will await the transaction receipt in a task, and send it.
                // This is done on a best effort basis. Errors won't be handled.
                if let Some(receipt_tx) = txn.receipt_tx {
                    let receipt_cache = self.receipt_cache.clone();
                    spawn!(
                        async move {
                            respond_with_receipt(
                                receipt_cache,
                                receipt_tx,
                                &txn.update_request,
                                TXN_RECEIPT_TIMEOUT,
                            )
                            .await;
                        },
                        "SIGNER: respond with receipt task"
                    );
                }
            }
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
                        self.query_runner
                            .simulate_txn(tx.update_request.clone().into()),
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
                    if let Err(e) =
                        send_to_forwarder(&self.mempool_socket, &pending_tx.update_request).await
                    {
                        error!("failed to send transaction to mempool: {e:?}");
                    } else {
                        pending_tx.tries += 1;
                    }
                }
            }
        }
    }
}

async fn send_to_forwarder(
    mempool_socket: &MempoolSocket,
    update_request: &UpdateRequest,
) -> anyhow::Result<()> {
    for _ in 0..MAX_FORWARDER_RETRIES {
        match mempool_socket.run(update_request.clone().into()).await {
            Ok(result) => match result {
                Ok(()) => return Ok(()),
                Err(e) => {
                    debug!("failed to send transaction to mempool: {e:?}");
                },
            },
            Err(e) => {
                debug!("failed to send transaction to mempool: {e:?}");
            },
        }
        tokio::time::sleep(WAIT_FOR_RETRY).await;
    }
    Err(anyhow!("stopped after {MAX_FORWARDER_RETRIES} tries"))
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

impl<C: NodeComponents> AsyncWorker for SignerWorker<C> {
    type Request = ExecuteTransaction;
    type Response = ();

    async fn handle(&mut self, request: ExecuteTransaction) {
        let mut state = self.state.lock().await;
        state.sign_new_tx(request).await;
    }
}

impl<C: NodeComponents> BuildGraph for Signer<C> {
    fn build_graph() -> fdi::DependencyGraph {
        fdi::DependencyGraph::new().with_infallible(
            Self::init.with_event_handler("start", Self::start.wrap_with_block_on()),
        )
    }
}

struct PendingTransaction {
    pub update_request: UpdateRequest,
    pub timestamp: SystemTime,
    pub tries: u8,
    pub receipt_tx: Option<oneshot::Sender<TransactionReceipt>>,
}

async fn new_block_task<C: NodeComponents>(
    mut node_index: LazyNodeIndex,
    worker: SignerWorker<C>,
    mut subscriber: impl Subscriber<BlockExecutedNotification>,
    query_runner: c![C::ApplicationInterface::SyncExecutor],
) {
    while let Some(_notification) = subscriber.last().await {
        let nonce = node_index.query_nonce(&query_runner);
        // TODO(qti3e): Get the lock only if we have to. Timeout should get sep from block.
        // Right now we are relying on the existence of new blocks to handle timeout.
        let mut guard = worker.state.lock().await;
        guard.sync_with_application(nonce).await;
    }
}

async fn respond_with_receipt(
    receipt_cache: Arc<Cache<[u8; 32], TransactionReceipt>>,
    receipt_tx: oneshot::Sender<TransactionReceipt>,
    txn: &UpdateRequest,
    timeout: Duration,
) {
    let mut interval = tokio::time::interval(TXN_RECEIPT_INTERVAL);
    let now = Instant::now();
    loop {
        if now.elapsed() >= timeout {
            tracing::debug!("Timeout while waiting for transaction receipt");
            break;
        }
        interval.tick().await;

        if let Some((_txn_hash, receipt)) = receipt_cache.remove(&txn.payload.to_digest()) {
            if let Err(e) = receipt_tx.send(receipt) {
                warn!("Failed to send transaction receipt: {e:?}");
            }
            break;
        }
    }
}
