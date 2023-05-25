use async_trait::async_trait;

use crate::{
    blockstore::Blake3Hash, common::WithStartAndShutdown, config::ConfigConsumer,
    reputation::ReputationQueryInteface,
};

#[async_trait]
pub trait IndexerInterface: WithStartAndShutdown + ConfigConsumer + Sized + Send + Sync {
    async fn init(config: Self::Config) -> anyhow::Result<Self>;

    /// Publish to everyone that we have cached a content with the given `cid` successfully.
    // TODO: Put the service that caused this cid to be cached as a param here.
    fn publish(&self, cid: &Blake3Hash);

    /// Returns the list of top nodes that should have a content cached.
    fn get_nodes_for_cid<Q: ReputationQueryInteface>(&self, reputation: &Q) -> Vec<u8>;
}
