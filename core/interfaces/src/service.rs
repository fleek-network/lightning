use async_trait::async_trait;
use infusion::c;
use tokio::net::UnixStream;

use crate::blockstore::BlockStoreInterface;
use crate::infu_collection::Collection;
use crate::types::ServiceId;
use crate::{
    ApplicationInterface,
    ConfigConsumer,
    ConfigProviderInterface,
    FetcherInterface,
    FetcherSocket,
    WithStartAndShutdown,
};

/// The service executor interface is responsible for loading the services and executing
/// these services.
///
/// Currently, we are hard coding some services and there is no API on this interface to
/// load services.
#[async_trait]
#[infusion::service]
pub trait ServiceExecutorInterface<C: Collection>:
    WithStartAndShutdown + ConfigConsumer + Sized + Send + Sync
{
    fn _init(
        config: ::ConfigProviderInterface,
        blockstore: ::BlockStoreInterface,
        fetcher: ::FetcherInterface,
        app: ::ApplicationInterface,
    ) {
        Self::init(
            config.get::<Self>(),
            blockstore,
            fetcher.get_socket(),
            app.sync_query(),
        )
    }

    /// The provider which can be used to get a handle on a service during runtime.
    type Provider: ExecutorProviderInterface;

    /// Initialize the service executor.
    fn init(
        config: Self::Config,
        blockstore: &C::BlockStoreInterface,
        fetcher_socket: FetcherSocket,
        query_runner: c!(C::ApplicationInterface::SyncExecutor),
    ) -> anyhow::Result<Self>;

    /// Returns the service handle provider which can be used establish connections to the
    /// services.
    fn get_provider(&self) -> Self::Provider;

    /// Run the code for the given service. This is a top level function that is assumed to
    /// take ownership over the entire binary. Must be called from the `main` function when
    /// the following environment variables exists:
    ///
    /// 1. `SERVICE_ID`
    /// 2. `BLOCKSTORE_PATH`
    /// 3. `IPC_PATH`
    fn run_service(id: u32);
}

#[async_trait]
#[infusion::blank]
pub trait ExecutorProviderInterface: Clone + Send + Sync + 'static {
    /// Make a connection to the provided service.
    async fn connect(&self, service_id: ServiceId) -> Option<UnixStream>;
}
