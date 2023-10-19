use affair::Socket;
use async_trait::async_trait;
use lightning_types::{FetcherRequest, FetcherResponse};

use crate::infu_collection::Collection;
use crate::{
    BlockStoreInterface,
    BlockStoreServerInterface,
    ConfigConsumer,
    ConfigProviderInterface,
    OriginProviderInterface,
    ResolverInterface,
    WithStartAndShutdown,
};

pub type FetcherSocket = Socket<FetcherRequest, FetcherResponse>;

#[async_trait]
#[infusion::service]
pub trait FetcherInterface<C: Collection>:
    WithStartAndShutdown + ConfigConsumer + Sized + Send + Sync
{
    fn _init(
        config: ::ConfigProviderInterface,
        blockstore: ::BlockStoreInterface,
        blockstore_server: ::BlockStoreServerInterface,
        resolver: ::ResolverInterface,
        origin: ::OriginProviderInterface,
    ) {
        Self::init(
            config.get::<Self>(),
            blockstore.clone(),
            blockstore_server,
            resolver.clone(),
            origin,
        )
    }

    /// Initialize the fetcher.
    fn init(
        config: Self::Config,
        blockstore: C::BlockStoreInterface,
        blockstore_server: &C::BlockStoreServerInterface,
        resolver: C::ResolverInterface,
        origin: &C::OriginProviderInterface,
    ) -> anyhow::Result<Self>;

    /// Returns a socket that can be used to submit requests to the fetcher.
    #[blank = FetcherSocket::raw_bounded(1).0]
    fn get_socket(&self) -> FetcherSocket;
}
