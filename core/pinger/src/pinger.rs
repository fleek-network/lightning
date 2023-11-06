use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use anyhow::anyhow;
use async_trait::async_trait;
use lightning_interfaces::infu_collection::{c, Collection};
use lightning_interfaces::types::NodeIndex;
use lightning_interfaces::{
    ApplicationInterface,
    ConfigConsumer,
    PingerInterface,
    ReputationAggregatorInterface,
    WithStartAndShutdown,
};
use tokio::net::UdpSocket;
use tokio::sync::broadcast;
use tracing::error;

use crate::config::Config;

pub struct Pinger<C: Collection> {
    config: Config,
    sender: Arc<PingSender<C>>,
    responder: Arc<PingResponder<C>>,
    is_running: Arc<AtomicBool>,
    shutdown_tx: broadcast::Sender<()>,
}

impl<C: Collection> PingerInterface<C> for Pinger<C> {
    fn init(
        config: Self::Config,
        query_runner: c!(C::ApplicationInterface::SyncExecutor),
        rep_reporter: c!(C::ReputationAggregatorInterface::ReputationReporter),
    ) -> anyhow::Result<Self> {
        let (tx, rx) = broadcast::channel(10);
        let sender = PingSender::<C>::new(
            query_runner.clone(),
            rep_reporter.clone(),
            Mutex::new(Some(rx)),
        );
        let responder =
            PingResponder::<C>::new(query_runner, rep_reporter, Mutex::new(Some(tx.subscribe())));
        Ok(Self {
            config,
            sender: Arc::new(sender),
            responder: Arc::new(responder),
            is_running: Arc::new(AtomicBool::new(false)),
            shutdown_tx: tx,
        })
    }
}

#[async_trait]
impl<C: Collection> WithStartAndShutdown for Pinger<C> {
    fn is_running(&self) -> bool {
        self.is_running.load(Ordering::Relaxed)
    }

    async fn start(&self) {
        // Note(matthias): providing the UPD socket to the sender and responder using interior
        // mutability is not elegant. It is necessary because the Pinger's init function is
        // not async, so we cannot create the socket in the init function and pass
        // it to the sender and responder there.
        // I didn't make the init function async because none of the other init functions are.
        if !self.is_running() {
            let sender = self.sender.clone();
            let responder = self.responder.clone();
            let socket = Arc::new(
                UdpSocket::bind(self.config.address)
                    .await
                    .expect("Failed to bind to UDP socket"),
            );
            sender.provide_socket(socket.clone());
            responder.provide_socket(socket);

            let is_running = self.is_running.clone();
            tokio::spawn(async move {
                sender.start().await;
                is_running.store(false, Ordering::Relaxed);
            });
            let is_running = self.is_running.clone();
            tokio::spawn(async move {
                responder.start().await;
                is_running.store(false, Ordering::Relaxed);
            });
            self.is_running.store(true, Ordering::Relaxed);
        } else {
            error!("Cannot start pinger because it is already running");
        }
    }

    async fn shutdown(&self) {
        self.shutdown_tx
            .send(())
            .expect("Failed to send shutdown signal");
    }
}

#[allow(unused)]
struct PingSender<C: Collection> {
    socket: Mutex<Option<Arc<UdpSocket>>>,
    query_runner: c!(C::ApplicationInterface::SyncExecutor),
    rep_reporter: c!(C::ReputationAggregatorInterface::ReputationReporter),
    shutdown_rx: Mutex<Option<broadcast::Receiver<()>>>,
}

impl<C: Collection> PingSender<C> {
    fn new(
        query_runner: c!(C::ApplicationInterface::SyncExecutor),
        rep_reporter: c!(C::ReputationAggregatorInterface::ReputationReporter),
        shutdown_rx: Mutex<Option<broadcast::Receiver<()>>>,
    ) -> Self {
        Self {
            socket: Mutex::new(None),
            query_runner,
            rep_reporter,
            shutdown_rx,
        }
    }

    async fn start(&self) {
        let _socket = self.socket.lock().unwrap().take().unwrap();
        let mut shutdown_rx = self.shutdown_rx.lock().unwrap().take().unwrap();
        loop {
            tokio::select! {
                _ = shutdown_rx.recv() => {
                    break;
                }

            }
        }

        *self.shutdown_rx.lock().unwrap() = Some(shutdown_rx);
    }

    fn provide_socket(&self, socket: Arc<UdpSocket>) {
        *self.socket.lock().unwrap() = Some(socket);
    }
}

#[allow(unused)]
struct PingResponder<C: Collection> {
    socket: Mutex<Option<Arc<UdpSocket>>>,
    query_runner: c!(C::ApplicationInterface::SyncExecutor),
    rep_reporter: c!(C::ReputationAggregatorInterface::ReputationReporter),
    shutdown_rx: Mutex<Option<broadcast::Receiver<()>>>,
}

impl<C: Collection> PingResponder<C> {
    fn new(
        query_runner: c!(C::ApplicationInterface::SyncExecutor),
        rep_reporter: c!(C::ReputationAggregatorInterface::ReputationReporter),
        shutdown_rx: Mutex<Option<broadcast::Receiver<()>>>,
    ) -> Self {
        Self {
            socket: Mutex::new(None),
            query_runner,
            rep_reporter,
            shutdown_rx,
        }
    }

    async fn start(&self) {
        let mut buf = [0; 1024];
        let socket = self.socket.lock().unwrap().take().unwrap();
        let mut shutdown_rx = self.shutdown_rx.lock().unwrap().take().unwrap();
        loop {
            tokio::select! {
                _ = shutdown_rx.recv() => {
                    break;
                }
                res = socket.recv_from(&mut buf) => {
                    match res {
                        Ok((len, addr)) => {
                            match PingMessage::try_from(&buf[0..len]) {
                                Ok(ping) => {
                                    // TODO(matthias): process ping message
                                    // TODO(matthias): use actual node index
                                    let pong = PingMessage { sender: u32::MAX, id: ping.id };
                                    let pong_bytes: Vec<u8> = pong.into();
                                    if let Err(e) = socket.send_to(&pong_bytes, addr).await {
                                        error!("Failed to respond to ping message: {e:?}");
                                    }
                                }
                                Err(e) => {
                                    error!("Failed to parse ping message: {e:?}");
                                }
                            }
                        }
                        Err(e) => {
                            error!("Failed to receive ping message: {e:?}");
                        }
                    }
                }
            }
        }

        *self.shutdown_rx.lock().unwrap() = Some(shutdown_rx);
    }

    fn provide_socket(&self, socket: Arc<UdpSocket>) {
        *self.socket.lock().unwrap() = Some(socket);
    }
}

impl<C: Collection> ConfigConsumer for Pinger<C> {
    const KEY: &'static str = "pinger";

    type Config = Config;
}

struct PingMessage {
    sender: NodeIndex,
    id: u32,
}

impl From<PingMessage> for Vec<u8> {
    fn from(value: PingMessage) -> Self {
        let mut buf = Vec::with_capacity(4);
        buf.extend_from_slice(&value.sender.to_le_bytes());
        buf.extend_from_slice(&value.id.to_le_bytes());
        buf
    }
}

impl TryFrom<&[u8]> for PingMessage {
    type Error = anyhow::Error;

    fn try_from(value: &[u8]) -> anyhow::Result<Self> {
        if value.len() != 8 {
            return Err(anyhow!("Number of bytes must be 8"));
        }
        Ok(Self {
            sender: NodeIndex::from_le_bytes(value[0..4].try_into()?),
            id: NodeIndex::from_le_bytes(value[4..8].try_into()?),
        })
    }
}
