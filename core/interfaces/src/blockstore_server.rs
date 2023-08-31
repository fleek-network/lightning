use anyhow::Result;
use async_trait::async_trait;
use infusion::c;
use lightning_types::NodeIndex;

use crate::infu_collection::Collection;
use crate::{
    ApplicationInterface,
    Blake3Hash,
    BlockStoreInterface,
    ConfigConsumer,
    ConfigProviderInterface,
    WithStartAndShutdown,
};

#[async_trait]
#[infusion::service]
pub trait BlockStoreServerInterface<C: Collection>:
    Clone + Send + Sync + ConfigConsumer + WithStartAndShutdown
{
    fn _init(
        config: ::ConfigProviderInterface,
        app: ::ApplicationInterface,
        blockstre: ::BlockStoreInterface,
    ) {
        Self::init(config.get::<Self>(), app.sync_query(), blockstre.clone())
    }

    fn init(
        config: Self::Config,
        query_runner: c!(C::ApplicationInterface::SyncExecutor),
        blockstore: C::BlockStoreInterface,
    ) -> anyhow::Result<Self>;

    async fn request_download(&self, block_hash: Blake3Hash, target: NodeIndex) -> Result<()>;
}
