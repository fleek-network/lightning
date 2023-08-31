use anyhow::Result;
use async_trait::async_trait;
use infusion::c;
use lightning_types::NodeIndex;

use crate::infu_collection::Collection;
use crate::{Blake3Hash, ConfigConsumer, WithStartAndShutdown};

#[async_trait]
#[infusion::service]
pub trait BlockStoreServerInterface<C: Collection>:
    Clone + Send + Sync + ConfigConsumer + WithStartAndShutdown
{
    fn init(
        config: Self::Config,
        query_runner: c!(C::ApplicationInterface::SyncExecutor),
        blockstore: C::BlockStoreInterface,
    ) -> anyhow::Result<Self>;

    async fn request_download(&self, block_hash: Blake3Hash, target: NodeIndex) -> Result<()>;
}
