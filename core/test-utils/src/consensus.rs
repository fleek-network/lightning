use std::collections::HashSet;
use std::marker::PhantomData;
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Duration;

use affair::{AsyncWorker, Executor, TokioSpawn};
use fdi::{BuildGraph, DependencyGraph};
use fleek_crypto::ConsensusPublicKey;
use lightning_interfaces::application::ExecutionEngineSocket;
// TODO(qti3e): Should we deprecate this?
use lightning_interfaces::config::ConfigConsumer;
use lightning_interfaces::consensus::ConsensusInterface;
use lightning_interfaces::infu_collection::{c, Collection};
use lightning_interfaces::types::{Block, Event, TransactionRequest};
use lightning_interfaces::{
    ApplicationInterface,
    BroadcastInterface,
    Emitter,
    ForwarderInterface,
    IndexSocket,
    MempoolSocket,
    NotifierInterface,
    SyncQueryRunnerInterface,
    WithStartAndShutdown,
};
use rand::Rng;
use serde::{Deserialize, Serialize};
use tokio::sync::{broadcast, mpsc};
use tokio::time::{interval, sleep};

static CHANNEL: OnceLock<broadcast::Sender<TransactionRequest>> = OnceLock::new();

/// A mock forwarder that sends transactions. MUST ALWAYS be used alongside [`MockConsensus`].
pub struct MockForwarder<C>(MempoolSocket, PhantomData<C>);
impl<C: Collection> ForwarderInterface<C> for MockForwarder<C> {
    fn init(
        _config: Self::Config,
        _consensus_key: ConsensusPublicKey,
        _query_runner: c!(C::ApplicationInterface::SyncExecutor),
    ) -> anyhow::Result<Self> {
        let tx = CHANNEL.get_or_init(|| broadcast::channel(128).0);
        let socket = TokioSpawn::spawn_async(MempoolSocketWorker { sender: tx.clone() });

        Ok(Self(socket, PhantomData))
    }

    fn mempool_socket(&self) -> MempoolSocket {
        self.0.clone()
    }
}

impl<C: Collection> BuildGraph for MockForwarder<C> {
    fn build_graph() -> DependencyGraph {
        DependencyGraph::new().with_infallible(|| {
            let tx = CHANNEL.get_or_init(|| broadcast::channel(128).0);
            let socket = TokioSpawn::spawn_async(MempoolSocketWorker { sender: tx.clone() });
            MockForwarder::<C>(socket, PhantomData)
        })
    }
}

impl<C> ConfigConsumer for MockForwarder<C> {
    const KEY: &'static str = "consensus";
    /// Just take the same config as mock consensus
    type Config = Config;
}

struct MempoolSocketWorker {
    sender: broadcast::Sender<TransactionRequest>,
}

impl AsyncWorker for MempoolSocketWorker {
    type Request = TransactionRequest;
    type Response = ();

    async fn handle(&mut self, req: Self::Request) -> Self::Response {
        let sender = self.sender.clone();
        sender
            .send(req)
            .expect("MockConsensus: Could not send to the consensus socket.");
    }
}

/// Mock consensus. MUST ALWAYS be used with [`MockForwarder`]
#[allow(clippy::type_complexity)]
pub struct MockConsensus<C: Collection> {
    inner: Arc<
        MockConsensusInner<
            c![C::ApplicationInterface::SyncExecutor],
            c![C::NotifierInterface::Emitter],
        >,
    >,
    is_running: Arc<Mutex<bool>>,
    shutdown_tx: Arc<Mutex<Option<mpsc::Sender<()>>>>,
    rx: Arc<Mutex<Option<broadcast::Receiver<TransactionRequest>>>>,
}

struct MockConsensusInner<Q: SyncQueryRunnerInterface + 'static, NE: Emitter> {
    _query_runner: Q,
    executor: ExecutionEngineSocket,
    config: Config,
    notifier: NE,
}

impl<C: Collection> ConsensusInterface<C> for MockConsensus<C> {
    type Certificate = ();

    /// Create a new consensus service with the provided config and executor.
    fn init(
        config: Self::Config,
        _keystore: C::KeystoreInterface,
        _signer: &C::SignerInterface,
        executor: ExecutionEngineSocket,
        query_runner: c!(C::ApplicationInterface::SyncExecutor),
        _pubsub: c!(C::BroadcastInterface::PubSub<Self::Certificate>),
        _indexer_socket: Option<IndexSocket>,
        notifier: &c!(C::NotifierInterface),
    ) -> anyhow::Result<Self> {
        let rx = CHANNEL
            .get_or_init(|| broadcast::channel(128).0)
            .subscribe();

        let inner = MockConsensusInner {
            _query_runner: query_runner,
            executor,
            config,
            notifier: notifier.get_emitter(),
        };
        Ok(Self {
            inner: Arc::new(inner),

            is_running: Arc::new(Mutex::new(false)),
            shutdown_tx: Arc::new(Mutex::new(None)),
            rx: Arc::new(Mutex::new(Some(rx))),
        })
    }

    fn set_event_tx(&mut self, _tx: mpsc::Sender<Vec<Event>>) {}
}

impl<C: Collection> WithStartAndShutdown for MockConsensus<C> {
    /// Returns true if this system is running or not.
    fn is_running(&self) -> bool {
        *self.is_running.lock().unwrap()
    }

    /// Start the system, should not do anything if the system is already
    /// started.
    async fn start(&self) {
        if !*self.is_running.lock().unwrap() {
            let (shutdown_tx, shutdown_rx) = mpsc::channel(1);
            let inner = self.inner.clone();
            let rx = self.rx.lock().unwrap().take().unwrap();
            tokio::spawn(async move { inner.handle(rx, shutdown_rx).await });
            *self.shutdown_tx.lock().unwrap() = Some(shutdown_tx);
            *self.is_running.lock().unwrap() = true;
        }
    }

    /// Send the shutdown signal to the system.
    async fn shutdown(&self) {
        let shutdown_tx = self.get_shutdown_tx();
        if let Some(shutdown_tx) = shutdown_tx {
            shutdown_tx.send(()).await.unwrap();
        }
        *self.is_running.lock().unwrap() = false;
    }
}

impl<C: Collection> MockConsensus<C> {
    fn get_shutdown_tx(&self) -> Option<mpsc::Sender<()>> {
        self.shutdown_tx.lock().unwrap().take()
    }
}

impl<C: Collection> ConfigConsumer for MockConsensus<C> {
    const KEY: &'static str = "consensus";

    type Config = Config;
}

impl<Q: SyncQueryRunnerInterface, NE: Emitter> MockConsensusInner<Q, NE> {
    async fn handle(
        self: Arc<Self>,
        mut rx: broadcast::Receiver<TransactionRequest>,
        mut shutdown_rx: mpsc::Receiver<()>,
    ) {
        let mut tx_count = 0;
        let mut interval = interval(self.config.new_block_interval);
        loop {
            tokio::select! {
                // fine to unwrap since the channel is static and will always be alive
                Ok(task) = rx.recv() => {
                    // Randomly wait before ordering the transaction to make it more realistic.
                    let range = self.config.min_ordering_time..self.config.max_ordering_time;
                    let ordering_duration = rand::thread_rng().gen_range(range);
                    sleep(Duration::from_secs(ordering_duration)).await;
                    // Randomly drop a transaction so we can handle this case.
                    if rand::thread_rng().gen_bool(self.config.probability_txn_lost) {
                        continue;
                    }
                    let update_request = task;
                    tx_count += 1;

                    if self.config.transactions_to_lose.contains(&tx_count) {
                        continue;
                    }

                    let block = Block {
                        transactions: vec![update_request],
                        digest: [0;32]
                    };

                    let _res = self.executor
                        .run(block)
                        .await
                        .map_err(|r| anyhow::anyhow!(format!("{r:?}")))
                        .unwrap();

                    self.notifier.new_block();
                }
                _ = interval.tick() => {
                    // Lets pretend that a new block arrived.
                    self.notifier.new_block();
                }
                _ = shutdown_rx.recv() => break,
            }
        }
    }
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
    pub new_block_interval: Duration,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            min_ordering_time: 0,
            max_ordering_time: 5,
            probability_txn_lost: 0.1,
            transactions_to_lose: HashSet::new(),
            new_block_interval: Duration::from_secs(5),
        }
    }
}
