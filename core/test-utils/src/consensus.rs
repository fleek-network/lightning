//! This module introduces a pair implementation of a consensus and forwarder component that must be
//! used together.
//!
//! Additionally it also exports a [MockConsensusGroup] for forcing multiple nodes to have the same
//! stream of 'blocks'.

use std::collections::HashSet;
use std::marker::PhantomData;
use std::sync::Arc;
use std::time::Duration;

use affair::AsyncWorkerUnordered;
use fdi::Cloned;
use lightning_interfaces::prelude::*;
use lightning_interfaces::spawn_worker;
use lightning_interfaces::types::{Block, TransactionRequest};
use rand::{thread_rng, Rng, SeedableRng};
use rand_chacha::ChaCha12Rng;
use rand_distr::{Bernoulli, Distribution};
use serde::{Deserialize, Serialize};
use tokio::sync::broadcast::error::RecvError;
use tokio::sync::{broadcast, mpsc};
use tokio::task::JoinSet;
use tokio::time::{interval, sleep};

/// The mock consensus group object is used to attach multiple mock nodes into the same 'consensus'
/// mechanism.
///
/// It can be viewed as a way to turn a [MockForwarder] of each one of the nodes using the same
/// group into a brodcast sender and the receiver will be all of the [ExecutionEngineSocket]s of
/// all of the nodes.
pub struct MockConsensusGroup {
    pub config: Config,
    req_tx: Option<mpsc::Sender<TransactionRequest>>,
    block_producer_rx: Option<broadcast::Receiver<Block>>,
    start: Option<Arc<tokio::sync::Notify>>,
}

impl MockConsensusGroup {
    pub fn new<Q: SyncQueryRunnerInterface>(
        config: Config,
        app_query: Option<Q>,
        start: Option<Arc<tokio::sync::Notify>>,
    ) -> Self {
        let (req_tx, req_rx) = mpsc::channel(128);
        let (block_producer_tx, block_producer_rx) = broadcast::channel(16);

        tokio::task::Builder::new()
            .name("MockConsensusGroup")
            .spawn(group_worker(
                config.clone(),
                app_query,
                start.clone(),
                req_rx,
                block_producer_tx,
            ))
            .unwrap();

        Self {
            config,
            req_tx: Some(req_tx),
            block_producer_rx: Some(block_producer_rx),
            start,
        }
    }

    pub fn start(&self) {
        if let Some(start) = self.start.clone() {
            start.notify_waiters();
        }
    }
}

impl Clone for MockConsensusGroup {
    fn clone(&self) -> Self {
        Self {
            config: self.config.clone(),
            req_tx: self.req_tx.clone(),
            block_producer_rx: self
                .block_producer_rx
                .as_ref()
                .map(broadcast::Receiver::resubscribe),
            start: self.start.clone(),
        }
    }
}

pub struct MockForwarder<C: NodeComponents> {
    socket: MempoolSocket,
    c: PhantomData<C>,
}

impl<C: NodeComponents> MockForwarder<C> {
    fn new(sender: mpsc::Sender<TransactionRequest>, waiter: ShutdownWaiter) -> Self {
        struct ProxyWorker(mpsc::Sender<TransactionRequest>);
        impl AsyncWorkerUnordered for ProxyWorker {
            type Request = TransactionRequest;
            type Response = ();
            async fn handle(&self, req: Self::Request) {
                self.0.send(req).await.expect("Failed to send transaction.")
            }
        }
        let worker = ProxyWorker(sender);
        let socket = spawn_worker!(worker, "MOCK-FORWARDER", waiter, crucial);
        Self {
            socket,
            c: PhantomData,
        }
    }
}

impl<C: NodeComponents> BuildGraph for MockForwarder<C> {
    fn build_graph() -> fdi::DependencyGraph {
        fdi::DependencyGraph::new().with_infallible(
            |mut group: fdi::RefMut<MockConsensusGroup>,
             fdi::Cloned(waiter): fdi::Cloned<ShutdownWaiter>| {
                Self::new(group.req_tx.take().unwrap(), waiter)
            },
        )
    }
}

impl<C: NodeComponents> ForwarderInterface<C> for MockForwarder<C> {
    fn mempool_socket(&self) -> MempoolSocket {
        self.socket.clone()
    }
}

/// Provides a controlled and mocked version of the consensus. Should be used in a collection with
/// [MockForwarder].
pub struct MockConsensus<C: NodeComponents> {
    group: broadcast::Receiver<Block>,
    execution_socket: ExecutionEngineSocket,
    notifier: c![C::NotifierInterface::Emitter],
}

impl<C: NodeComponents> MockConsensus<C> {
    pub fn new(
        app: &C::ApplicationInterface,
        notifier: &c!(C::NotifierInterface),
        mut group: fdi::RefMut<MockConsensusGroup>,
    ) -> Self {
        let notifier = notifier.get_emitter();
        Self {
            group: group.block_producer_rx.take().unwrap(),
            execution_socket: app.transaction_executor(),
            notifier,
        }
    }

    async fn start(mut this: fdi::Consume<Self>, Cloned(waiter): Cloned<ShutdownWaiter>) {
        waiter
            .run_until_shutdown(async move {
                loop {
                    match this.group.recv().await {
                        Ok(block) => {
                            let response = this
                                .execution_socket
                                .run(block.clone())
                                .await
                                .map_err(|r| anyhow::anyhow!(format!("{r:?}")))
                                .unwrap();
                            this.notifier.new_block(block, response);
                        },
                        Err(RecvError::Closed) => break,
                        Err(RecvError::Lagged(_)) => continue,
                    }
                }
            })
            .await;
    }
}

impl<C: NodeComponents> ConsensusInterface<C> for MockConsensus<C> {
    type Certificate = ();
    type ReadyState = ();

    async fn wait_for_ready(&self) -> Self::ReadyState {}
}

impl<C: NodeComponents> BuildGraph for MockConsensus<C> {
    fn build_graph() -> fdi::DependencyGraph {
        fdi::DependencyGraph::new()
            .with_infallible(|
                config: fdi::Ref<C::ConfigProviderInterface>,
                fdi::Cloned(app_query): fdi::Cloned<c![C::ApplicationInterface::SyncExecutor]>
            | {
                MockConsensusGroup::new(config.get::<Self>(), Some(app_query), None)
            })
            .with_infallible(
                Self::new.with_event_handler(
                    "start",
                    Self::start.wrap_with_spawn_named("MOCK-CONSENSUS"),
                ),
            )
    }
}

impl<C: NodeComponents> ConfigConsumer for MockConsensus<C> {
    const KEY: &'static str = "mock_consensus";
    type Config = Config;
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Config {
    /// Lower bound for the random time it takes to order a transaction.
    pub min_ordering_time: u64,
    /// Upper bound for the random time it takes to order a transaction.
    pub max_ordering_time: u64,
    /// Probability that a transaction won't get through.
    /// The nonce won't be incremented on the application.
    pub probability_txn_lost: f64,
    /// Transactions specified in this set will be lost.
    /// For example, if the set contains 1 and 3, then the first and third transactions
    /// arriving at the consensus will be lost.
    pub transactions_to_lose: HashSet<u32>,
    /// This specifies the interval for new blocks being pretend submitted to the application.
    #[serde(with = "humantime_serde")]
    pub new_block_interval: Duration,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            min_ordering_time: 0,
            max_ordering_time: 5,
            probability_txn_lost: 0.0,
            transactions_to_lose: HashSet::new(),
            new_block_interval: Duration::from_secs(5),
        }
    }
}

async fn group_worker<Q: SyncQueryRunnerInterface>(
    config: Config,
    app_query: Option<Q>,
    start: Option<Arc<tokio::sync::Notify>>,
    mut req_rx: mpsc::Receiver<TransactionRequest>,
    block_producer_tx: broadcast::Sender<Block>,
) {
    // Wait for genesis if app query is given.
    if let Some(app_query) = app_query {
        app_query.wait_for_genesis().await;
    }

    // Wait for the start signal.
    if let Some(start) = start {
        start.notified().await;
    }

    let period = if config.new_block_interval.is_zero() {
        Duration::from_secs(120)
    } else {
        config.new_block_interval
    };
    let mut interval = interval(period);
    let mut tx_count = 0;

    // After each tx is received we want to add some random delay to it to make it more
    // realistic.
    let mut delayed_queue = JoinSet::new();

    // This is a hack to force the JoinSet to never return `None`. This simplifies the
    // tokio::select. Maybe there is utility future somewhere
    delayed_queue.spawn(futures::future::pending::<TransactionRequest>());

    let mut loss_prob_rng = ChaCha12Rng::from_seed(thread_rng().gen());
    let loss_prob_distr = Bernoulli::new(config.probability_txn_lost).unwrap();

    let mut payload = Vec::with_capacity(1024);
    let mut prev_digest = [0; 32];

    loop {
        let mut block = tokio::select! {
            Some(req) = delayed_queue.join_next() => {
                Block {
                    transactions: vec![req.unwrap()],
                    digest: [0; 32],
                    sub_dag_index: 0,
                    sub_dag_round: 0,
                }
            },
            Some(req) = req_rx.recv() => {
                tx_count += 1;

                if config.transactions_to_lose.contains(&tx_count) {
                    tracing::info!("losing transaction {}: {:?}", tx_count, req);
                    continue;
                }

                if loss_prob_distr.sample(&mut loss_prob_rng) {
                    continue;
                }

                // Randomly wait before ordering the transaction to make it more realistic.
                let range = config.min_ordering_time..config.max_ordering_time;
                delayed_queue.spawn(async move {
                    if !range.is_empty() && config.max_ordering_time > 0 {
                        let ordering_delay = rand::thread_rng().gen_range(range);
                        if ordering_delay > 0 {
                            sleep(Duration::from_secs(ordering_delay)).await;
                        }
                    }
                    req
                });

                continue;
            },
            _ = interval.tick() => {
                if config.new_block_interval.is_zero() {
                    continue;
                }
                Block {
                    transactions: vec![],
                    digest: [0; 32],
                    sub_dag_index: 0,
                    sub_dag_round: 0,
                }
            },
            else => {
                break;
            }
        };

        // Compute the mock block digest.
        payload.clear();
        payload.extend(&prev_digest);
        for tx in &block.transactions {
            payload.extend(tx.hash());
        }
        block.digest = *fleek_blake3::hash(&payload).as_bytes();
        prev_digest = block.digest;

        if block_producer_tx.send(block).is_err() {
            return;
        }
    }
}
