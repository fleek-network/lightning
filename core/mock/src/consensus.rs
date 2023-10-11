use std::marker::PhantomData;
use std::net::SocketAddr;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use affair::{AsyncWorker, Executor, TokioSpawn};
use async_trait::async_trait;
use axum::extract::State;
use axum::routing::post;
use axum::{Json, Router};
use lightning_interfaces::infu_collection::{c, Collection};
use lightning_interfaces::types::{Block, TransactionRequest};
use lightning_interfaces::{
    ApplicationInterface,
    BroadcastInterface,
    ConfigConsumer,
    ConsensusInterface,
    ExecutionEngineSocket,
    IndexSocket,
    MempoolSocket,
    SignerInterface,
    WithStartAndShutdown,
};
use log::{debug, info};
use rand::{thread_rng, Rng, SeedableRng};
use rand_chacha::ChaCha20Rng;
use rand_distr::{Binomial, Cauchy, Distribution};
use serde::{Deserialize, Serialize};
use tokio::sync::{mpsc, Notify};

/// A mock consensus that listens to an HTTP endpoint and accepts transactions using its `/tx`
/// post endpoint. Transactions can be sent as JSON values.
///
/// The mempool it provides also has a configurable success rate that must be from 0.0 to 1.0.
pub struct MockConsensus<C: Collection> {
    addr: SocketAddr,
    socket: mpsc::Sender<TransactionRequest>,
    is_running: Arc<AtomicBool>,
    shutdown_notifier: Arc<Notify>,
    block_notifier: Arc<Notify>,
    mempool: MempoolSocket,
    collection: PhantomData<C>,
}

#[derive(Serialize, Deserialize)]
pub struct Config {
    pub port: u16,
    host: std::net::IpAddr,
    mempool_success_rate: f64,
}

struct MempoolSocketWorker {
    sender: mpsc::Sender<TransactionRequest>,
    success_distr: Binomial,
    delay_distr: Cauchy<f64>,
    rng: ChaCha20Rng,
}

#[async_trait]
impl AsyncWorker for MempoolSocketWorker {
    type Request = TransactionRequest;
    type Response = ();

    async fn handle(&mut self, req: Self::Request) -> Self::Response {
        let success = self.success_distr.sample(&mut self.rng) >= 50;

        if !success {
            debug!("Failed to submit transaction.");
            return;
        }

        let sender = self.sender.clone();
        let delay = self.delay_distr.sample(&mut self.rng);

        // simulate some sort of delay around 1s.
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_micros((delay * 1000.0) as u64)).await;

            sender
                .send(req)
                .await
                .expect("MockConsensus: Could not send to the consensus socket.");
        });
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            // 69 in base 3
            port: 2120,
            host: std::net::IpAddr::V4(std::net::Ipv4Addr::new(0, 0, 0, 0)),
            mempool_success_rate: 0.85,
        }
    }
}

#[async_trait]
impl<C: Collection> WithStartAndShutdown for MockConsensus<C> {
    fn is_running(&self) -> bool {
        self.is_running.load(Ordering::Relaxed)
    }

    async fn start(&self) {
        if self.is_running() {
            return;
        }

        self.is_running.store(true, Ordering::Relaxed);

        let is_running = self.is_running.clone();
        let shutdown_notifier = self.shutdown_notifier.clone();
        let addr = self.addr;

        let state = Arc::new(self.socket.clone());
        let app = Router::new()
            .route(
                "/tx",
                post(
                    |State(sender): State<Arc<mpsc::Sender<TransactionRequest>>>,
                     Json(payload): Json<TransactionRequest>| async move {
                        sender.send(payload).await.expect(
                            "MockConsensus: Could not send HTTP request through the sender.",
                        );

                        "OK"
                    },
                ),
            )
            .with_state(state);

        tokio::spawn(async move {
            info!("MockConsensus listening on {addr}");

            axum::Server::bind(&addr)
                .serve(app.into_make_service())
                .with_graceful_shutdown(shutdown_notifier.notified())
                .await
                .expect("MockConsensus: Failed to setup the server.");

            is_running.store(false, Ordering::Relaxed);
        });
    }

    /// Send the shutdown signal to the system.
    async fn shutdown(&self) {
        self.shutdown_notifier.notify_waiters();
    }
}

#[async_trait]
impl<C: Collection> ConsensusInterface<C> for MockConsensus<C> {
    type Certificate = ();

    fn init<S: SignerInterface<C>>(
        config: Self::Config,
        _signer: &S,
        executor: ExecutionEngineSocket,
        _query_runner: c!(C::ApplicationInterface::SyncExecutor),
        _pubsub: c!(C::BroadcastInterface::PubSub<Self::Certificate>),
        _indexer_socket: Option<IndexSocket>,
    ) -> anyhow::Result<Self> {
        let (tx, rx) = mpsc::channel(128);
        let block_notifier = Arc::new(Notify::new());
        tokio::spawn(run_consensus(rx, executor, block_notifier.clone()));

        let mempool = TokioSpawn::spawn_async(MempoolSocketWorker {
            sender: tx.clone(),
            success_distr: Binomial::new(100, config.mempool_success_rate)
                .expect("MockConsensus: Success rate must be 0.0<=p<=1.0"),
            delay_distr: Cauchy::new(1000.0, 0.3).unwrap(), // 1000 is in ms.
            rng: ChaCha20Rng::from_seed(thread_rng().gen()),
        });

        let addr = SocketAddr::new(config.host, config.port);
        Ok(Self {
            addr,
            socket: tx,
            is_running: Arc::new(AtomicBool::new(false)),
            shutdown_notifier: Arc::new(Notify::new()),
            block_notifier,
            mempool,
            collection: PhantomData,
        })
    }

    fn mempool(&self) -> MempoolSocket {
        self.mempool.clone()
    }

    fn new_block_notifier(&self) -> Arc<Notify> {
        self.block_notifier.clone()
    }
}

impl<C: Collection> ConfigConsumer for MockConsensus<C> {
    const KEY: &'static str = "consensus";

    type Config = Config;
}

async fn run_consensus(
    mut rx: mpsc::Receiver<TransactionRequest>,
    exec: ExecutionEngineSocket,
    block_notifier: Arc<Notify>,
) {
    let mut ticker = tokio::time::interval(Duration::from_millis(1));
    let mut buffer = Vec::<TransactionRequest>::with_capacity(32);

    loop {
        tokio::select! {
            _ = ticker.tick() => {
                if buffer.is_empty() {
                    continue;
                }

                let transactions = std::mem::replace(&mut buffer, Vec::with_capacity(32));
                let block = Block {
                    transactions,
                    digest: [0;32]
                };

                let _ = exec.run(block).await;

                block_notifier.notify_waiters();
            }
            tmp = rx.recv() => {
                if let Some(tx) = tmp {
                    buffer.push(tx);
                } else {
                    // There is nothing coming anymore.
                    return;
                }
            }
        }
    }
}
