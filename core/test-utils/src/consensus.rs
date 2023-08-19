use std::collections::HashSet;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use affair::{Socket, Task};
use async_trait::async_trait;
use lightning_interfaces::application::{ExecutionEngineSocket, SyncQueryRunnerInterface};
use lightning_interfaces::config::ConfigConsumer;
use lightning_interfaces::consensus::{ConsensusInterface, MempoolSocket};
use lightning_interfaces::infu_collection::{c, Collection};
use lightning_interfaces::signer::SignerInterface;
use lightning_interfaces::types::{Block, UpdateRequest};
use lightning_interfaces::{
    ApplicationInterface,
    BroadcastInterface,
    PubSub,
    WithStartAndShutdown,
};
use rand::Rng;
use serde::{Deserialize, Serialize};
use tokio::sync::{mpsc, Notify};
use tokio::time::{interval, sleep};

#[derive(Serialize, Deserialize, Clone)]
pub struct MockPubSub {}

#[async_trait]
impl PubSub<()> for MockPubSub {
    /// Publish a message.
    async fn send(&self, _msg: &()) {}

    /// Await the next message in the topic, should only return `None` if there are
    /// no longer any new messages coming. (indicating that the gossip instance is
    /// shutdown.)
    async fn recv(&mut self) -> Option<()> {
        None
    }
}

// TODO(qti3e): Should we deprecate this?
#[allow(clippy::type_complexity)]
pub struct MockConsensus<C: Collection> {
    inner: Arc<MockConsensusInner<c![C::ApplicationInterface::SyncExecutor]>>,
    socket: Socket<UpdateRequest, ()>,
    is_running: Arc<Mutex<bool>>,
    shutdown_tx: Arc<Mutex<Option<mpsc::Sender<()>>>>,
    rx: Arc<Mutex<Option<mpsc::Receiver<Task<UpdateRequest, ()>>>>>,
    new_block_notify: Arc<Notify>,
}

struct MockConsensusInner<Q: SyncQueryRunnerInterface + 'static> {
    _query_runner: Q,
    executor: ExecutionEngineSocket,
    config: Config,
    new_block_notify: Arc<Notify>,
}

#[async_trait]
impl<C: Collection> ConsensusInterface<C> for MockConsensus<C> {
    type Certificate = ();

    /// Create a new consensus service with the provided config and executor.
    fn init<S: SignerInterface<C>>(
        config: Self::Config,
        _signer: &S,
        executor: ExecutionEngineSocket,
        query_runner: c!(C::ApplicationInterface::SyncExecutor),
        _pubsub: c!(C::BroadcastInterface::PubSub<Self::Certificate>),
    ) -> anyhow::Result<Self> {
        let (socket, rx) = Socket::raw_bounded(2048);
        let new_block_notify = Arc::new(Notify::new());
        let inner = MockConsensusInner {
            _query_runner: query_runner,
            executor,
            config,
            new_block_notify: new_block_notify.clone(),
        };
        Ok(Self {
            inner: Arc::new(inner),
            socket,
            is_running: Arc::new(Mutex::new(false)),
            shutdown_tx: Arc::new(Mutex::new(None)),
            rx: Arc::new(Mutex::new(Some(rx))),
            new_block_notify,
        })
    }

    /// Returns a socket that can be used to submit transactions to the consensus,
    /// this can be used by any other systems that are interested in posting some
    /// transaction to the consensus.
    fn mempool(&self) -> MempoolSocket {
        self.socket.clone()
    }

    fn new_block_notifier(&self) -> Arc<Notify> {
        self.new_block_notify.clone()
    }
}

#[async_trait]
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

impl<Q: SyncQueryRunnerInterface> MockConsensusInner<Q> {
    async fn handle(
        self: Arc<Self>,
        mut rx: mpsc::Receiver<Task<UpdateRequest, ()>>,
        mut shutdown_rx: mpsc::Receiver<()>,
    ) {
        let mut tx_count = 0;
        let mut interval = interval(self.config.new_block_interval);
        loop {
            tokio::select! {
                task = rx.recv() => {
                    let task = task.expect("Failed to receive UpdateRequest.");
                    // Randomly wait before ordering the transaction to make it more realistic.
                    let range = self.config.min_ordering_time..self.config.max_ordering_time;
                    let ordering_duration = rand::thread_rng().gen_range(range);
                    sleep(Duration::from_secs(ordering_duration)).await;
                    // Randomly drop a transaction so we can handle this case.
                    if rand::thread_rng().gen_bool(self.config.probability_txn_lost) {
                        continue;
                    }
                    let update_request = task.request.clone();
                    task.respond(());
                    tx_count += 1;

                    if self.config.transactions_to_lose.contains(&tx_count) {
                        continue;
                    }

                    let block = Block {
                        transactions: vec![update_request],
                    };

                    let _res = self.executor
                        .run(block)
                        .await
                        .map_err(|r| anyhow::anyhow!(format!("{r:?}")))
                        .unwrap();

                    self.new_block_notify.notify_waiters();
                }
                _ = interval.tick() => {
                    // Lets pretend that a new block arrived.
                    self.new_block_notify.notify_waiters();
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
