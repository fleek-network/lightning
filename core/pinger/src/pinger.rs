// TODO(qti3e): Someone has to rework this file.
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::anyhow;
use fleek_crypto::NodePublicKey;
use lightning_interfaces::prelude::*;
use lightning_interfaces::types::{NodeIndex, NodeInfo};
use lightning_metrics::histogram;
use lightning_utils::application::QueryRunnerExt;
use rand::rngs::SmallRng;
use rand::seq::SliceRandom;
use rand::{Rng, SeedableRng};
use tokio::net::UdpSocket;
use tokio::sync::mpsc;
use tracing::{error, info};
use types::{ProtocolParamKey, ProtocolParamValue};

use crate::config::Config;

pub struct Pinger<C: NodeComponents> {
    inner: Option<PingerInner<C>>,
}

impl<C: NodeComponents> Pinger<C> {
    pub fn new(
        config_provider: &C::ConfigProviderInterface,
        app: &C::ApplicationInterface,
        rep_aggregator: &C::ReputationAggregatorInterface,
        keystore: &C::KeystoreInterface,
        fdi::Cloned(notifier): fdi::Cloned<C::NotifierInterface>,
        fdi::Cloned(shutdown_waiter): fdi::Cloned<ShutdownWaiter>,
    ) -> anyhow::Result<Self> {
        let config = config_provider.get::<Self>();
        let query_runner = app.sync_query();
        let rep_reporter = rep_aggregator.get_reporter();

        let node_pk = keystore.get_ed25519_pk();
        let inner = PingerInner::<C>::new(
            config,
            node_pk,
            query_runner,
            rep_reporter,
            notifier,
            shutdown_waiter,
        );

        Ok(Self { inner: Some(inner) })
    }

    pub async fn start(mut this: fdi::RefMut<Self>) {
        this.inner
            .take()
            .expect("Pinger already started")
            .run()
            .await;
    }
}

impl<C: NodeComponents> PingerInterface<C> for Pinger<C> {}

impl<C: NodeComponents> BuildGraph for Pinger<C> {
    fn build_graph() -> fdi::DependencyGraph {
        fdi::DependencyGraph::new().with(
            Self::new.with_event_handler("start", Self::start.wrap_with_spawn_named("PINGER")),
        )
    }
}

struct PingerInner<C: NodeComponents> {
    config: Config,
    node_pk: NodePublicKey,
    query_runner: c!(C::ApplicationInterface::SyncExecutor),
    rep_reporter: c!(C::ReputationAggregatorInterface::ReputationReporter),
    notifier: C::NotifierInterface,
    shutdown_waiter: ShutdownWaiter,
}

impl<C: NodeComponents> PingerInner<C> {
    fn new(
        config: Config,
        node_pk: NodePublicKey,
        query_runner: c!(C::ApplicationInterface::SyncExecutor),
        rep_reporter: c!(C::ReputationAggregatorInterface::ReputationReporter),
        notifier: C::NotifierInterface,
        shutdown_waiter: ShutdownWaiter,
    ) -> Self {
        Self {
            config,
            node_pk,
            query_runner,
            rep_reporter,
            notifier,
            shutdown_waiter,
        }
    }

    async fn run(self) {
        // Note(matthias): should a node be able to respond to pings before it knows its node index?
        // In my opinion it should not because it is not fully functioning.
        let mut interval = tokio::time::interval(Duration::from_secs(60));
        let node_index = loop {
            tokio::select! {
                _ = self.shutdown_waiter.wait_for_shutdown() => {
                    return;
                }
                _ = interval.tick() => {
                    if let Some(index) = self.query_runner.pubkey_to_index(&self.node_pk) {
                        break index;
                    }
                }
            }
        };

        // At this point we can assume that the node knows its index.

        let mut buf = [0; 1024];
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
        let mut epoch_changed_notifier = self.notifier.subscribe_epoch_changed();

        loop {
            tokio::select! {
                _ = self.shutdown_waiter.wait_for_shutdown() => {
                    return;
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
                                                let rtt = instant.elapsed() / 2;

                                                histogram!(
                                                    "pinger_latency",
                                                    Some("Histogram of all pinger latency measurements"),
                                                    rtt.as_millis() as f64 / 1000f64
                                                );

                                                self.rep_reporter
                                                    .report_ping(sender, Some(rtt));
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
                    let peer_index = node_registry[cursor];
                    cursor = (cursor + 1) % node_registry.len();
                    if let Some(node) =
                        self.query_runner.get_node_info::<NodeInfo>(&peer_index, |n| n) {
                        let id = rng.gen_range(0..u32::MAX);
                        let msg = Message::Request { sender: node_index, id };
                        let bytes: Vec<u8> = msg.into();
                        let addr = (node.domain, node.ports.pinger);
                        let timeout = self.get_ping_timeout();
                        if let Err(e) = socket.send_to(&bytes, addr).await {
                            error!("Failed to respond to ping message: {e:?}");
                        } else {
                            pending_req.insert((peer_index, id), Instant::now());
                            let tx = timeout_tx.clone();
                            spawn!(async move {
                                tokio::time::sleep(timeout).await;
                                // We ignore the sending error because it can happen that the
                            "PINGER: request timeout");
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
                Some(_) = epoch_changed_notifier.recv() => {
                    info!("Configuring for new epoch");
                    node_registry = self.get_node_registry(&mut rng);
                    cursor = 0;
                }
                else => {
                    break;
                }
            }
        }
    }

    fn get_node_registry(&self, rng: &mut SmallRng) -> Vec<NodeIndex> {
        let mut nodes: Vec<NodeIndex> = self
            .query_runner
            .get_active_nodes()
            .into_iter()
            .map(|node| node.index)
            .collect();
        nodes.shuffle(rng);
        nodes
    }

    fn get_ping_timeout(&self) -> Duration {
        match self
            .query_runner
            .get_protocol_param(&ProtocolParamKey::ReputationPingTimeout)
        {
            Some(ProtocolParamValue::ReputationPingTimeout(timeout)) => timeout,
            // Return 15s if the param is not set, for backwards compatibility.
            None => Duration::from_secs(15),
            _ => unreachable!(),
        }
    }
}

impl<C: NodeComponents> ConfigConsumer for Pinger<C> {
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
        if value.len() != 9 {
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
