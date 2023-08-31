use std::net::SocketAddr;

use anyhow::Result;
use async_trait::async_trait;
use lightning_types::NodeIndex;

use crate::infu_collection::Collection;
use crate::{
    Blake3Hash,
    BlockStoreInterface,
    ConfigConsumer,
    ConfigProviderInterface,
    SyncQueryRunnerInterface,
    WithStartAndShutdown,
};

#[async_trait]
#[infusion::service]
pub trait BlockStoreServerInterface<C: Collection>:
    Clone + Send + Sync + ConfigConsumer + WithStartAndShutdown
{
    fn _init(config: ::ConfigProviderInterface, blockstre: ::BlockStoreInterface) {
        Self::init(config.get::<Self>(), blockstre.clone())
    }

    fn init(config: Self::Config, blockstore: C::BlockStoreInterface) -> anyhow::Result<Self>;

    fn extract_address<Q: SyncQueryRunnerInterface>(
        query_runner: Q,
        target: NodeIndex,
    ) -> Option<SocketAddr>;

    async fn request_download(&self, block_hash: Blake3Hash, target: SocketAddr) -> Result<()>;
}
