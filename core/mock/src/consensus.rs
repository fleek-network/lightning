use std::marker::PhantomData;
use std::net::SocketAddr;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, OnceLock};
use std::time::Duration;

use affair::{AsyncWorker, Executor, TokioSpawn};
use axum::extract::State;
use axum::routing::post;
use axum::{Json, Router};
use fleek_crypto::ConsensusPublicKey;
use lightning_interfaces::infu_collection::{c, Collection};
use lightning_interfaces::types::{Block, Event, TransactionRequest};
use lightning_interfaces::{
    ApplicationInterface,
    BroadcastInterface,
    ConfigConsumer,
    ConsensusInterface,
    Emitter,
    ExecutionEngineSocket,
    ForwarderInterface,
    IndexSocket,
    MempoolSocket,
    NotifierInterface,
    WithStartAndShutdown,
};
use rand::{thread_rng, Rng, SeedableRng};
use rand_chacha::ChaCha20Rng;
use rand_distr::{Binomial, Cauchy, Distribution};
use serde::{Deserialize, Serialize};
use tokio::sync::{broadcast, mpsc, Notify};
use tracing::{debug, info};

static CHANNEL: OnceLock<broadcast::Sender<TransactionRequest>> = OnceLock::new();

/// Shared config between mock consensus and forwarder
#[derive(Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub port: u16,
    host: std::net::IpAddr,
    mempool_success_rate: f64,
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

/// A mock forwarder that sends transactions with configurable random distribution of success and
/// delay. MUST ALWAYS be used alongside [`MockConsensus`].
pub struct MockForwarder<C>(MempoolSocket, PhantomData<C>);
impl<C: Collection> ForwarderInterface<C> for MockForwarder<C> {
    fn init(
        config: Self::Config,
        _consensus_key: ConsensusPublicKey,
        _query_runner: c!(C::ApplicationInterface::SyncExecutor),
    ) -> anyhow::Result<Self> {
        let tx = CHANNEL.get_or_init(|| broadcast::channel(128).0);
        let socket = TokioSpawn::spawn_async(MempoolSocketWorker {
            sender: tx.clone(),
            success_distr: Binomial::new(100, config.mempool_success_rate)
                .expect("MockConsensus: Success rate must be 0.0<=p<=1.0"),
            delay_distr: Cauchy::new(1000.0, 0.3).unwrap(), // 1000 is in ms.
            rng: ChaCha20Rng::from_seed(thread_rng().gen()),
        });

        Ok(Self(socket, PhantomData))
    }

    fn mempool_socket(&self) -> MempoolSocket {
        self.0.clone()
    }
}

impl<C> ConfigConsumer for MockForwarder<C> {
    const KEY: &'static str = "consensus";
    /// Just take the same config as mock consensus
    type Config = Config;
}

struct MempoolSocketWorker {
    sender: broadcast::Sender<TransactionRequest>,
    success_distr: Binomial,
    delay_distr: Cauchy<f64>,
    rng: ChaCha20Rng,
}

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
                .expect("MockConsensus: Could not send to the consensus socket.");
        });
    }
}

/// A mock consensus that listens to an HTTP endpoint and accepts transactions using its `/tx`
/// post endpoint. Transactions can be sent as JSON values.
///
/// The mempool it provides also has a configurable success rate that must be from 0.0 to 1.0.
///
/// This MUST always be used along with the [`MockForwarder`]
pub struct MockConsensus<C: Collection> {
    addr: SocketAddr,
    socket: broadcast::Sender<TransactionRequest>,
    is_running: Arc<AtomicBool>,
    shutdown_notifier: Arc<Notify>,
    collection: PhantomData<C>,
}

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
                    |State(sender): State<Arc<broadcast::Sender<TransactionRequest>>>,
                     Json(payload): Json<TransactionRequest>| async move {
                        sender.send(payload).expect(
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

impl<C: Collection> ConsensusInterface<C> for MockConsensus<C> {
    type Certificate = ();

    fn init(
        config: Self::Config,
        _keystore: C::KeystoreInterface,
        _signer: &C::SignerInterface,
        executor: ExecutionEngineSocket,
        _query_runner: c!(C::ApplicationInterface::SyncExecutor),
        _pubsub: c!(C::BroadcastInterface::PubSub<Self::Certificate>),
        _indexer_socket: Option<IndexSocket>,
        notifier: &c!(C::NotifierInterface),
    ) -> anyhow::Result<Self> {
        let tx = CHANNEL.get_or_init(|| broadcast::channel(128).0);

        tokio::spawn(run_consensus::<C>(
            tx.subscribe(),
            executor,
            notifier.get_emitter(),
        ));

        let addr = SocketAddr::new(config.host, config.port);
        Ok(Self {
            addr,
            socket: tx.clone(),
            is_running: Arc::new(AtomicBool::new(false)),
            shutdown_notifier: Arc::new(Notify::new()),
            collection: PhantomData,
        })
    }

    fn set_event_tx(&mut self, _tx: mpsc::Sender<Vec<Event>>) {}
}

impl<C: Collection> ConfigConsumer for MockConsensus<C> {
    const KEY: &'static str = "consensus";
    type Config = Config;
}

async fn run_consensus<C: Collection>(
    mut rx: broadcast::Receiver<TransactionRequest>,
    exec: ExecutionEngineSocket,
    notifier: c!(C::NotifierInterface::Emitter),
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

                notifier.new_block();
            }
            tmp = rx.recv() => {
                if let Ok(tx) = tmp {
                    buffer.push(tx);
                } else {
                    // There is nothing coming anymore.
                    return;
                }
            }
        }
    }
}
