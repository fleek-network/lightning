use affair::Socket;
use anyhow::Result;
use async_trait::async_trait;
use lightning_types::{PeerRequestError, ServerRequest};
use tokio::sync::broadcast;

use crate::infu_collection::Collection;
use crate::{
    BlockStoreInterface,
    ConfigConsumer,
    ConfigProviderInterface,
    PoolInterface,
    WithStartAndShutdown,
};

pub type BlockStoreServerSocket =
    Socket<ServerRequest, broadcast::Receiver<Result<(), PeerRequestError>>>;

#[async_trait]
#[infusion::service]
pub trait BlockStoreServerInterface<C: Collection>:
    Sized + Send + Sync + ConfigConsumer + WithStartAndShutdown
{
    fn _init(
        config: ::ConfigProviderInterface,
        blockstre: ::BlockStoreInterface,
        pool: ::PoolInterface,
    ) {
        Self::init(config.get::<Self>(), blockstre.clone(), pool)
    }

    fn init(
        config: Self::Config,
        blockstore: C::BlockStoreInterface,
        pool: &C::PoolInterface,
    ) -> anyhow::Result<Self>;

    fn get_socket(&self) -> BlockStoreServerSocket;
}
