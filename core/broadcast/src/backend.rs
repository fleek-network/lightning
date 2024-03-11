use std::future::Future;

use bytes::Bytes;
use fleek_crypto::NodePublicKey;
use lightning_interfaces::types::NodeIndex;
use lightning_interfaces::Weight;

pub trait BroadcastBackend: Send + Sync + 'static {
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
}
