use std::path::PathBuf;

use async_trait::async_trait;
use tokio::net::UnixStream;

use crate::blockstore::BlockStoreInterface;
use crate::infu_collection::Collection;
use crate::types::ServiceId;
use crate::{ConfigConsumer, ConfigProviderInterface, WithStartAndShutdown};

/// The service executor interface is responsible for loading the services and executing
/// these services.
///
/// Currently, we are hard coding some services and there is no API on this interface to
/// load services.
#[infusion::service]
pub trait ServiceExecutorInterface<C: Collection>:
    WithStartAndShutdown + ConfigConsumer + Sized + Send + Sync
{
    fn _init(config: ::ConfigProviderInterface, blockstore: ::BlockStoreInterface) {
        Self::init(config.get::<Self>(), blockstore)
    }

    /// The provider which can be used to get a handle on a service during runtime.
    type Provider: ExecutorProviderInterface;

    /// Initialize the service executor.
    fn init(config: Self::Config, blockstore: &C::BlockStoreInterface) -> anyhow::Result<Self>;

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
    fn run_service(id: u32, blockstore_path: PathBuf, ipc_path: PathBuf);
}

#[async_trait]
#[infusion::blank]
pub trait ExecutorProviderInterface: Clone + Send + Sync + 'static {
    /// Make a connection to the provided service.
    async fn connect(&self, service_id: ServiceId) -> Option<UnixStream>;
}
