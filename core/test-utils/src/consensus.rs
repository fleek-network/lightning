use std::{
    sync::{Arc, Mutex},
    time::Duration,
};

use affair::{Socket, Task};
use async_trait::async_trait;
use draco_interfaces::{
    application::{ExecutionEngineSocket, SyncQueryRunnerInterface},
    config::ConfigConsumer,
    consensus::{ConsensusInterface, MempoolSocket},
    signer::SignerInterface,
    types::{Block, UpdateRequest},
    WithStartAndShutdown,
};
use rand::Rng;
use serde::{Deserialize, Serialize};
use tokio::{sync::mpsc, time::sleep};

#[allow(clippy::type_complexity)]
pub struct MockConsensus<Q: SyncQueryRunnerInterface + 'static> {
    inner: Arc<MockConsensusInner<Q>>,
    socket: Socket<UpdateRequest, ()>,
    is_running: Arc<Mutex<bool>>,
    shutdown_tx: Arc<Mutex<Option<mpsc::Sender<()>>>>,
    rx: Arc<Mutex<Option<mpsc::Receiver<Task<UpdateRequest, ()>>>>>,
}

struct MockConsensusInner<Q: SyncQueryRunnerInterface + 'static> {
    _query_runner: Q,
    executor: ExecutionEngineSocket,
    config: Config,
}

#[async_trait]
impl<Q: SyncQueryRunnerInterface> ConsensusInterface for MockConsensus<Q> {
    type QueryRunner = Q;

    /// Create a new consensus service with the provided config and executor.
    async fn init<S: SignerInterface>(
        config: Config,
        _signer: &S,
        executor: ExecutionEngineSocket,
        query_runner: Self::QueryRunner,
    ) -> anyhow::Result<Self> {
        let (socket, rx) = Socket::raw_bounded(2048);
        let inner = MockConsensusInner {
            _query_runner: query_runner,
            executor,
            config,
        };
        Ok(Self {
            inner: Arc::new(inner),
            socket,
            is_running: Arc::new(Mutex::new(false)),
            shutdown_tx: Arc::new(Mutex::new(None)),
            rx: Arc::new(Mutex::new(Some(rx))),
        })
    }

    /// Returns a socket that can be used to submit transactions to the consensus,
    /// this can be used by any other systems that are interested in posting some
    /// transaction to the consensus.
    fn mempool(&self) -> MempoolSocket {
        self.socket.clone()
    }
}

#[async_trait]
impl<Q: SyncQueryRunnerInterface + 'static> WithStartAndShutdown for MockConsensus<Q> {
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
    }
}

impl<Q: SyncQueryRunnerInterface> MockConsensus<Q> {
    fn get_shutdown_tx(&self) -> Option<mpsc::Sender<()>> {
        self.shutdown_tx.lock().unwrap().take()
    }
}

impl<Q: SyncQueryRunnerInterface> ConfigConsumer for MockConsensus<Q> {
    const KEY: &'static str = "consensus";

    type Config = Config;
}

impl<Q: SyncQueryRunnerInterface> MockConsensusInner<Q> {
    async fn handle(
        self: Arc<Self>,
        mut rx: mpsc::Receiver<Task<UpdateRequest, ()>>,
        mut shutdown_rx: mpsc::Receiver<()>,
    ) {
        loop {
            //let mut nonces = Arc::new(scc::HashMap::new());
            tokio::select! {
                task = rx.recv() => {
                    let task = task.expect("Failed to receive UpdateRequest.");
                    // Randomly wait before ordering the transaction to make it more realistic.
                    let range = self.config.min_ordering_time..self.config.max_ordering_time;
                    let ordering_duration = rand::thread_rng().gen_range(range);
                    sleep(Duration::from_secs(ordering_duration)).await;
                    // Randomly drop a transaction so we can handle this case.
                    if rand::thread_rng().gen_bool(self.config.probability_transaction_lost) {
                        continue;
                    }
                    let update_request = task.request.clone();
                    task.respond(());

                    let block = Block {
                        transactions: vec![update_request],
                    };

                    let _res = self.executor
                        .run(block)
                        .await
                        .map_err(|r| anyhow::anyhow!(format!("{r:?}")))
                        .unwrap();

                }
                _ = shutdown_rx.recv() => break,
            }
        }
    }
}

#[derive(Serialize, Deserialize)]
pub struct Config {
    pub min_ordering_time: u64,
    pub max_ordering_time: u64,
    pub probability_transaction_lost: f64,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            min_ordering_time: 0,
            max_ordering_time: 5,
            probability_transaction_lost: 0.1,
        }
    }
}
