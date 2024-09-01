use std::collections::HashSet;
use std::future::Future;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use bytes::Bytes;
use fleek_crypto::{NodePublicKey, NodeSecretKey, NodeSignature, PublicKey, SecretKey};
use lightning_interfaces::prelude::*;
use lightning_interfaces::types::NodeIndex;
use lightning_interfaces::Weight;
use tokio::sync::mpsc;

pub trait BroadcastBackend: 'static {
    type Pk;

    fn send_to_all<F: Fn(NodeIndex) -> bool + Send + Sync + 'static>(
        &self,
        payload: Bytes,
        filter: F,
    );

    fn send_to_one(&self, node: NodeIndex, payload: Bytes);

    fn receive(&mut self) -> impl Future<Output = Option<(NodeIndex, Bytes)>> + Send;

    fn get_node_pk(&self, index: NodeIndex) -> Option<Self::Pk>;

    fn get_our_index(&self) -> Option<NodeIndex>;

    fn sign(&self, digest: [u8; 32]) -> NodeSignature;

    fn verify(pk: &Self::Pk, signature: &NodeSignature, digest: &[u8; 32]) -> bool;

    fn report_sat(&self, peer: NodeIndex, weight: Weight);

    fn now() -> u64;

    fn sleep(duration: Duration) -> impl Future<Output = ()> + Send;
}

pub struct LightningBackend<C: NodeComponents> {
    sqr: c![C::ApplicationInterface::SyncExecutor],
    rep_reporter: c![C::ReputationAggregatorInterface::ReputationReporter],
    event_handler: c![C::PoolInterface::EventHandler],
    sk: NodeSecretKey,
    pk: NodePublicKey,
}

impl<C: NodeComponents> LightningBackend<C> {
    pub fn new(
        sqr: c![C::ApplicationInterface::SyncExecutor],
        rep_reporter: c![C::ReputationAggregatorInterface::ReputationReporter],
        event_handler: c![C::PoolInterface::EventHandler],
        sk: NodeSecretKey,
    ) -> Self {
        let pk = sk.to_pk();
        Self {
            sqr,
            rep_reporter,
            event_handler,
            sk,
            pk,
        }
    }
}

impl<C: NodeComponents> BroadcastBackend for LightningBackend<C> {
    type Pk = NodePublicKey;

    fn send_to_all<F: Fn(NodeIndex) -> bool + Send + Sync + 'static>(
        &self,
        payload: Bytes,
        filter: F,
    ) {
        self.event_handler.send_to_all(payload, filter)
    }

    #[inline(always)]
    fn send_to_one(&self, node: NodeIndex, payload: Bytes) {
        self.event_handler.send_to_one(node, payload)
    }

    #[inline(always)]
    fn receive(&mut self) -> impl Future<Output = Option<(NodeIndex, Bytes)>> + Send {
        self.event_handler.receive()
    }

    #[inline(always)]
    fn get_node_pk(&self, index: NodeIndex) -> Option<NodePublicKey> {
        self.sqr.index_to_pubkey(&index)
    }

    #[inline(always)]
    fn get_our_index(&self) -> Option<NodeIndex> {
        self.sqr.pubkey_to_index(&self.pk)
    }

    #[inline(always)]
    fn sign(&self, digest: [u8; 32]) -> NodeSignature {
        self.sk.sign(&digest)
    }

    #[inline(always)]
    fn verify(pk: &Self::Pk, signature: &NodeSignature, digest: &[u8; 32]) -> bool {
        pk.verify(signature, digest).expect("invalid signature")
    }

    #[inline(always)]
    fn report_sat(&self, peer: NodeIndex, weight: Weight) {
        self.rep_reporter.report_sat(peer, weight)
    }

    /// Get the current unix timestamp in milliseconds
    #[inline(always)]
    fn now() -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64
    }

    #[inline(always)]
    async fn sleep(duration: Duration) {
        tokio::time::sleep(duration).await;
    }
}

pub struct SimulonBackend {
    peers: HashSet<usize>,
    msg_tx: mpsc::Sender<(usize, Bytes)>,
    msg_rx: mpsc::Receiver<(NodeIndex, Bytes)>,
}

impl SimulonBackend {
    pub fn new(
        msg_tx: mpsc::Sender<(usize, Bytes)>,
        msg_rx: mpsc::Receiver<(NodeIndex, Bytes)>,
        peers: HashSet<usize>,
    ) -> Self {
        Self {
            peers,
            msg_tx,
            msg_rx,
        }
    }
}

impl BroadcastBackend for SimulonBackend {
    type Pk = NodeIndex;

    fn send_to_all<F: Fn(NodeIndex) -> bool + Send + Sync + 'static>(
        &self,
        payload: Bytes,
        filter: F,
    ) {
        for peer in &self.peers {
            if filter(*peer as NodeIndex) {
                let tx = self.msg_tx.clone();
                let payload_ = payload.clone();
                let peer = *peer;
                simulon::api::spawn(async move {
                    tx.send((peer, payload_))
                        .await
                        .expect("Failed to send message");
                });
            }
        }
    }

    fn send_to_one(&self, node: NodeIndex, payload: Bytes) {
        let tx = self.msg_tx.clone();
        simulon::api::spawn(async move {
            tx.send((node as usize, payload))
                .await
                .expect("Failed to send message");
        });
    }

    fn receive(&mut self) -> impl Future<Output = Option<(NodeIndex, Bytes)>> + Send {
        self.msg_rx.recv()
    }

    #[inline(always)]
    fn get_node_pk(&self, index: NodeIndex) -> Option<Self::Pk> {
        Some(index)
    }

    #[inline(always)]
    fn get_our_index(&self) -> Option<NodeIndex> {
        Some(*simulon::api::RemoteAddr::whoami() as NodeIndex)
    }

    #[inline(always)]
    fn sign(&self, _digest: [u8; 32]) -> NodeSignature {
        NodeSignature([0; 64])
    }

    #[inline(always)]
    fn verify(_pk: &Self::Pk, _signature: &NodeSignature, _digest: &[u8; 32]) -> bool {
        true
    }

    #[inline(always)]
    fn report_sat(&self, _peer: NodeIndex, _weight: Weight) {}

    #[inline(always)]
    fn now() -> u64 {
        (simulon::api::now() / 1_000_000) as u64
    }

    #[inline(always)]
    fn sleep(duration: Duration) -> impl Future<Output = ()> + Send {
        simulon::api::sleep(duration)
    }
}
