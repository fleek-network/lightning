use async_trait::async_trait;
use lightning_interfaces::{
    common::WithStartAndShutdown, config::ConfigConsumer, indexer::IndexerInterface, Blake3Hash,
    ReputationQueryInteface,
};

use super::config::Config;

#[derive(Clone)]
pub struct Indexer {}

#[async_trait]
impl WithStartAndShutdown for Indexer {
    /// Returns true if this system is running or not.
    fn is_running(&self) -> bool {
        true
    }

    /// Start the system, should not do anything if the system is already
    /// started.
    async fn start(&self) {}

    /// Send the shutdown signal to the system.
    async fn shutdown(&self) {}
}

#[async_trait]
impl IndexerInterface for Indexer {
    async fn init(_config: Self::Config) -> anyhow::Result<Self> {
        Ok(Self {})
    }

    /// Publish to everyone that we have cached a content with the given `cid` successfully.
    // TODO: Put the service that caused this cid to be cached as a param here.
    fn publish(&self, _cid: &Blake3Hash) {
        todo!()
    }

    /// Returns the list of top nodes that should have a content cached.
    fn get_nodes_for_cid<Q: ReputationQueryInteface>(&self, _reputation: &Q) -> Vec<u8> {
        todo!()
    }
}

impl ConfigConsumer for Indexer {
    const KEY: &'static str = "indexer";

    type Config = Config;
}
