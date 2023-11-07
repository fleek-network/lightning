use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Instant;

use anyhow::anyhow;
use async_trait::async_trait;
use lightning_interfaces::infu_collection::{c, Collection};
use lightning_interfaces::types::NodeIndex;
use lightning_interfaces::{
    ApplicationInterface,
    ConfigConsumer,
    Notification,
    NotifierInterface,
    PingerInterface,
    ReputationAggregatorInterface,
    ReputationReporterInterface,
    SyncQueryRunnerInterface,
    WithStartAndShutdown,
};
use rand::rngs::SmallRng;
use rand::seq::SliceRandom;
use rand::{Rng, SeedableRng};
use tokio::net::UdpSocket;
use tokio::sync::{broadcast, mpsc};
use tracing::error;

use crate::config::Config;

pub struct Pinger<C: Collection> {
    inner: Arc<PingerInner<C>>,
    is_running: Arc<AtomicBool>,
    shutdown_tx: broadcast::Sender<()>,
}

impl<C: Collection> PingerInterface<C> for Pinger<C> {
    fn init(
        config: Self::Config,
        query_runner: c!(C::ApplicationInterface::SyncExecutor),
        rep_reporter: c!(C::ReputationAggregatorInterface::ReputationReporter),
        notifier: C::NotifierInterface,
    ) -> anyhow::Result<Self> {
        let (notifier_tx, notifier_rx) = mpsc::channel(10);
        notifier.notify_on_new_epoch(notifier_tx);

        let (shutdown_tx, shutdown_rx) = broadcast::channel(10);
        let inner = PingerInner::<C>::new(
            config,
            query_runner,
            rep_reporter,
            notifier_rx,
            Mutex::new(Some(shutdown_rx)),
        );
        Ok(Self {
            inner: Arc::new(inner),
            is_running: Arc::new(AtomicBool::new(false)),
            shutdown_tx,
        })
    }
}

#[async_trait]
impl<C: Collection> WithStartAndShutdown for Pinger<C> {
    fn is_running(&self) -> bool {
        self.is_running.load(Ordering::Relaxed)
    }

    async fn start(&self) {
        if !self.is_running() {
            let inner = self.inner.clone();
            let is_running = self.is_running.clone();
            tokio::spawn(async move {
                inner.start().await;
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
struct PingerInner<C: Collection> {
    config: Config,
    query_runner: c!(C::ApplicationInterface::SyncExecutor),
    rep_reporter: c!(C::ReputationAggregatorInterface::ReputationReporter),
    notifier_rx: Mutex<Option<mpsc::Receiver<Notification>>>,
    shutdown_rx: Mutex<Option<broadcast::Receiver<()>>>,
}

impl<C: Collection> PingerInner<C> {
    fn new(
        config: Config,
        query_runner: c!(C::ApplicationInterface::SyncExecutor),
        rep_reporter: c!(C::ReputationAggregatorInterface::ReputationReporter),
        notifier_rx: mpsc::Receiver<Notification>,
        shutdown_rx: Mutex<Option<broadcast::Receiver<()>>>,
    ) -> Self {
        Self {
            config,
            query_runner,
            rep_reporter,
            notifier_rx: Mutex::new(Some(notifier_rx)),
            shutdown_rx,
        }
    }

    async fn start(&self) {
        let mut buf = [0; 1024];
        let mut shutdown_rx = self.shutdown_rx.lock().unwrap().take().unwrap();
        let mut notifier_rx = self.notifier_rx.lock().unwrap().take().unwrap();

        let socket = Arc::new(
            UdpSocket::bind(self.config.address)
                .await
                .expect("Failed to bind to UDP socket"),
        );

        // Note(matthias): instead of having a fixed interval, we can also determine how many nodes
        // there are, how many pings we want to send to each node per epoch, and the epoch length,
        // and then set the interval accordingly.
        // The advantage of setting a fixed interval is that we can control the network traffic.
        // The advantage of setting the interval dynamically is that ensures that every node is
        // pinged x amount of times per epoch.
        let mut interval = tokio::time::interval(self.config.ping_interval);

        let mut rng = SmallRng::from_entropy();
        let mut node_registry = self.get_node_registry(&mut rng);
        let mut cursor = 0;

        let mut pending_req: HashMap<(NodeIndex, u32), Instant> = HashMap::with_capacity(128);
        loop {
            tokio::select! {
                _ = shutdown_rx.recv() => {
                    break;
                }
                res = socket.recv_from(&mut buf) => {
                    match res {
                        Ok((len, addr)) => {
                            match Message::try_from(&buf[0..len]) {
                                Ok(msg) => {
                                    // TODO(matthias): process ping message
                                    // TODO(matthias): use actual node index
                                    match msg {
                                        Message::Request { sender: _, id } => {
                                            let pong = Message::Response { sender: u32::MAX, id };
                                            let bytes: Vec<u8> = pong.into();
                                            if let Err(e) = socket.send_to(&bytes, addr).await {
                                                error!("Failed to respond to ping message: {e:?}");
                                            }
                                        }
                                        Message::Response { sender, id } => {
                                            // TODO(matthias): should we use signatures to prove
                                            // a message was sent from the sender?
                                            // Should we make sure the sender IP matches the IP on
                                            // the app state?
                                            let key = &(sender, id); // thank you clippy
                                            if let Some(instant) = pending_req.remove(key) {
                                                let rtt = instant.elapsed();
                                                self.rep_reporter
                                                    .report_ping(sender, Some(rtt / 2));
                                            }
                                        }
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
                _ = interval.tick() => {
                    let peer_index = node_registry[cursor % node_registry.len()];
                    if let Some(node) = self.query_runner.get_node_info_with_index(&peer_index) {
                        let id = rng.gen_range(0..u32::MAX);
                        let msg = Message::Request { sender: u32::MAX, id };
                        let bytes: Vec<u8> = msg.into();
                        let addr = (node.domain, node.ports.pinger);
                        if let Err(e) = socket.send_to(&bytes, addr).await {
                            error!("Failed to respond to ping message: {e:?}");
                        } else {
                            pending_req.insert((peer_index, id), Instant::now());
                        }
                    }
                }
                epoch_notify = notifier_rx.recv() => {
                    if let Some(Notification::NewEpoch) = epoch_notify {
                        node_registry = self.get_node_registry(&mut rng);
                        cursor = 0;
                    }
                }
            }
        }

        *self.shutdown_rx.lock().unwrap() = Some(shutdown_rx);
        *self.notifier_rx.lock().unwrap() = Some(notifier_rx);
    }

    fn get_node_registry(&self, rng: &mut SmallRng) -> Vec<NodeIndex> {
        let mut nodes: Vec<NodeIndex> = self
            .query_runner
            .get_node_registry(None)
            .into_iter()
            .filter_map(|node| self.query_runner.pubkey_to_index(node.public_key))
            .collect();
        nodes.shuffle(rng);
        nodes
    }
}

impl<C: Collection> ConfigConsumer for Pinger<C> {
    const KEY: &'static str = "pinger";

    type Config = Config;
}

enum Message {
    Request { sender: NodeIndex, id: u32 },
    Response { sender: NodeIndex, id: u32 },
}

impl From<Message> for Vec<u8> {
    fn from(value: Message) -> Self {
        let mut buf = Vec::with_capacity(9);
        match value {
            Message::Request { sender, id } => {
                buf.push(0x00);
                buf.extend_from_slice(&sender.to_le_bytes());
                buf.extend_from_slice(&id.to_le_bytes());
            },
            Message::Response { sender, id } => {
                buf.push(0x01);
                buf.extend_from_slice(&sender.to_le_bytes());
                buf.extend_from_slice(&id.to_le_bytes());
            },
        }
        buf
    }
}

impl TryFrom<&[u8]> for Message {
    type Error = anyhow::Error;

    fn try_from(value: &[u8]) -> anyhow::Result<Self> {
        if value.len() != 8 {
            return Err(anyhow!("Number of bytes must be 9"));
        }
        match &value[0] {
            0x00 => Ok(Self::Request {
                sender: NodeIndex::from_le_bytes(value[1..5].try_into()?),
                id: NodeIndex::from_le_bytes(value[5..9].try_into()?),
            }),
            0x01 => Ok(Self::Response {
                sender: NodeIndex::from_le_bytes(value[1..5].try_into()?),
                id: NodeIndex::from_le_bytes(value[5..9].try_into()?),
            }),
            _ => Err(anyhow!("Invalid magic byte")),
        }
    }
}
