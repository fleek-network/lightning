use std::future::Future;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use bytes::Bytes;
use fleek_crypto::NodePublicKey;
use infusion::c;
use lightning_interfaces::infu_collection::Collection;
use lightning_interfaces::types::NodeIndex;
use lightning_interfaces::{
    ApplicationInterface,
    EventHandlerInterface,
    PoolInterface,
    ReputationAggregatorInterface,
    ReputationReporterInterface,
    SyncQueryRunnerInterface,
    Weight,
};

pub trait BroadcastBackend: 'static {
    fn send_to_all<F: Fn(NodeIndex) -> bool + Send + Sync + 'static>(
        &self,
        payload: Bytes,
        filter: F,
    );
    fn send_to_one(&self, node: NodeIndex, payload: Bytes);

    //async fn receive(&mut self) -> Option<(NodeIndex, Bytes)>;
    fn receive(&mut self) -> impl Future<Output = Option<(NodeIndex, Bytes)>> + Send;

    fn get_node_pk(&self, index: NodeIndex) -> Option<NodePublicKey>;

    fn get_node_index(&self, node: &NodePublicKey) -> Option<NodeIndex>;

    fn report_sat(&self, peer: NodeIndex, weight: Weight);

    fn now() -> u64;

    fn sleep(duration: Duration) -> impl Future<Output = ()> + Send;
}

pub struct LightningBackend<C: Collection> {
    sqr: c![C::ApplicationInterface::SyncExecutor],
    rep_reporter: c![C::ReputationAggregatorInterface::ReputationReporter],
    event_handler: c![C::PoolInterface::EventHandler],
}

impl<C: Collection> LightningBackend<C> {
    pub fn new(
        sqr: c![C::ApplicationInterface::SyncExecutor],
        rep_reporter: c![C::ReputationAggregatorInterface::ReputationReporter],
        event_handler: c![C::PoolInterface::EventHandler],
    ) -> Self {
        Self {
            sqr,
            rep_reporter,
            event_handler,
        }
    }
}

impl<C: Collection> BroadcastBackend for LightningBackend<C> {
    fn send_to_all<F: Fn(NodeIndex) -> bool + Send + Sync + 'static>(
        &self,
        payload: Bytes,
        filter: F,
    ) {
        self.event_handler.send_to_all(payload, filter)
    }

    fn send_to_one(&self, node: NodeIndex, payload: Bytes) {
        self.event_handler.send_to_one(node, payload)
    }

    fn receive(&mut self) -> impl Future<Output = Option<(NodeIndex, Bytes)>> + Send {
        self.event_handler.receive()
    }

    fn get_node_pk(&self, index: NodeIndex) -> Option<NodePublicKey> {
        self.sqr.index_to_pubkey(&index)
    }

    fn get_node_index(&self, node: &NodePublicKey) -> Option<NodeIndex> {
        self.sqr.pubkey_to_index(node)
    }

    fn report_sat(&self, peer: NodeIndex, weight: Weight) {
        self.rep_reporter.report_sat(peer, weight)
    }

    /// Get the current unix timestamp in milliseconds
    fn now() -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64
    }

    async fn sleep(duration: Duration) {
        tokio::time::sleep(duration).await;
    }
}
