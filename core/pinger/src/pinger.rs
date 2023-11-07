use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use anyhow::anyhow;
use async_trait::async_trait;
use fleek_crypto::{NodeSecretKey, SecretKey};
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
    SignerInterface,
    SyncQueryRunnerInterface,
    WithStartAndShutdown,
};
use rand::rngs::SmallRng;
use rand::seq::SliceRandom;
use rand::{Rng, SeedableRng};
use tokio::net::UdpSocket;
use tokio::sync::{mpsc, Notify, OnceCell};
use tracing::{error, info};

use crate::config::Config;

/// The duration after which a ping will be reported as unanswered
const TIMEOUT: Duration = Duration::from_secs(15);

pub struct Pinger<C: Collection> {
    inner: Arc<PingerInner<C>>,
    is_running: Arc<AtomicBool>,
    shutdown_notify: Arc<Notify>,
}

impl<C: Collection> PingerInterface<C> for Pinger<C> {
    fn init(
        config: Self::Config,
        query_runner: c!(C::ApplicationInterface::SyncExecutor),
        rep_reporter: c!(C::ReputationAggregatorInterface::ReputationReporter),
        notifier: C::NotifierInterface,
        signer: &C::SignerInterface,
    ) -> anyhow::Result<Self> {
        let (notifier_tx, notifier_rx) = mpsc::channel(10);
        notifier.notify_on_new_epoch(notifier_tx);

        let shutdown_notify = Arc::new(Notify::new());
        let (_, node_sk) = signer.get_sk();
        let inner = PingerInner::<C>::new(
            config,
            node_sk,
            query_runner,
            rep_reporter,
            notifier_rx,
            shutdown_notify.clone(),
        );
        Ok(Self {
            inner: Arc::new(inner),
            is_running: Arc::new(AtomicBool::new(false)),
            shutdown_notify,
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
        self.shutdown_notify.notify_one();
    }
}

#[allow(unused)]
struct PingerInner<C: Collection> {
    config: Config,
    node_index: OnceCell<NodeIndex>,
    node_sk: NodeSecretKey,
    query_runner: c!(C::ApplicationInterface::SyncExecutor),
    rep_reporter: c!(C::ReputationAggregatorInterface::ReputationReporter),
    notifier_rx: Mutex<Option<mpsc::Receiver<Notification>>>,
    shutdown_notify: Arc<Notify>,
}

impl<C: Collection> PingerInner<C> {
    fn new(
        config: Config,
        node_sk: NodeSecretKey,
        query_runner: c!(C::ApplicationInterface::SyncExecutor),
        rep_reporter: c!(C::ReputationAggregatorInterface::ReputationReporter),
        notifier_rx: mpsc::Receiver<Notification>,
        shutdown_notify: Arc<Notify>,
    ) -> Self {
        Self {
            config,
            node_index: OnceCell::new(),
            node_sk,
            query_runner,
            rep_reporter,
            notifier_rx: Mutex::new(Some(notifier_rx)),
            shutdown_notify,
        }
    }

    async fn start(&self) {
        let mut buf = [0; 1024];
        let mut notifier_rx = self.notifier_rx.lock().unwrap().take().unwrap();
        let (timeout_tx, mut timeout_rx) = mpsc::channel(1024);

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

        // Node(matthias): Should a node be able to respond to pings before it knows its node index?
        // In my opinion it should not because it is not fully functioning.

        if !self.node_index.initialized() {
            loop {
                if let Some(index) = self.query_runner.pubkey_to_index(self.node_sk.to_pk()) {
                    self.node_index.set(index).expect("Failed to set once cell");
                    break;
                }
                tokio::time::sleep(Duration::from_secs(60)).await;
            }
        }
        // At this point we can assume that the node knows its index.
        let node_index = *self.node_index.get().unwrap();

        loop {
            tokio::select! {
                _ = self.shutdown_notify.notified() => {
                    break;
                }
                res = socket.recv_from(&mut buf) => {
                    match res {
                        Ok((len, addr)) => {
                            match Message::try_from(&buf[0..len]) {
                                Ok(msg) => {
                                    match msg {
                                        Message::Request { sender: _, id } => {
                                            // TODO(matthias): verify sender before responding?
                                            let pong = Message::Response { sender: node_index, id };
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
                        let msg = Message::Request { sender: node_index, id };
                        let bytes: Vec<u8> = msg.into();
                        let addr = (node.domain, node.ports.pinger);
                        if let Err(e) = socket.send_to(&bytes, addr).await {
                            error!("Failed to respond to ping message: {e:?}");
                        } else {
                            pending_req.insert((peer_index, id), Instant::now());
                            let tx = timeout_tx.clone();
                            tokio::spawn(async move {
                                tokio::time::sleep(TIMEOUT).await;
                                tx.send((peer_index, id)).await.expect("Failed to send timeout");
                            });
                        }
                    }
                }
                timeout = timeout_rx.recv() => {
                    if let Some((node, id)) = timeout {
                        if pending_req.remove(&(node, id)).is_some() {
                            // Report unanswered ping
                            self.rep_reporter
                                .report_ping(node, None);
                        }
                    }
                }
                epoch_notify = notifier_rx.recv() => {
                    if let Some(Notification::NewEpoch) = epoch_notify {
                        info!("Configuring for new epoch");
                        node_registry = self.get_node_registry(&mut rng);
                        cursor = 0;
                    }
                }
            }
        }

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
