use std::marker::PhantomData;
use std::ops::DerefMut;
use std::sync::Arc;

use jsonrpsee::server::middleware::ProxyGetRequestLayer;
use jsonrpsee::server::{Server as JSONRPCServer, ServerHandle};
use jsonrpsee::RpcModule;
use lightning_interfaces::infu_collection::{c, Collection};
use lightning_interfaces::{
    ApplicationInterface,
    ArchiveSocket,
    ConfigConsumer,
    FetcherInterface,
    FetcherSocket,
    MempoolSocket,
    RpcInterface,
    WithStartAndShutdown,
};
use tokio::sync::Mutex;
use tower::layer::util::{Identity, Stack};

pub use crate::api::{EthApiServer, FleekApiServer, NetApiServer};
pub use crate::config::Config;
pub use crate::logic::{EthApi, FleekApi, NetApi};
pub mod api;
mod api_types;
pub mod config;
mod error;
mod logic;
pub mod utils;

#[cfg(test)]
mod tests;

type Server = JSONRPCServer<Stack<ProxyGetRequestLayer, Stack<ProxyGetRequestLayer, Identity>>>;

pub(crate) struct Data<C: Collection> {
    pub query_runner: c!(C::ApplicationInterface::SyncExecutor),
    pub mempool_socket: MempoolSocket,
    pub fetcher_socket: FetcherSocket,
    pub _blockstore: C::BlockStoreInterface,
    /// If this is some it means the node is in archive mode
    pub archive_socket: Option<ArchiveSocket>,
}

pub struct Rpc<C: Collection> {
    config: Config,
    /// The final RPCModule containting selected methods
    module: RpcModule<()>,
    // need interior mutability to support restarts
    handle: Mutex<Option<ServerHandle>>,

    phantom: std::marker::PhantomData<C>,
}

impl<C: Collection> Rpc<C> {
    async fn build_server_from_config(&self) -> anyhow::Result<Server> {
        let builder = JSONRPCServer::builder();

        let heatlh_intercept = ProxyGetRequestLayer::new("/health", "flk_health")?;
        let metrics_intercept = ProxyGetRequestLayer::new("/metrics", "flk_metrics")?;

        let middleware = tower::ServiceBuilder::new()
            .layer(heatlh_intercept)
            .layer(metrics_intercept);

        Ok(builder
            .set_middleware(middleware)
            .build(self.config.addr())
            .await?)
    }

    fn create_modules_from_config(
        config: &Config,
        data: Arc<Data<C>>,
    ) -> anyhow::Result<RpcModule<()>> {
        let mut final_module = RpcModule::new(());

        for selection in config.rpc_selection() {
            match selection {
                config::RPCModules::Eth => {
                    final_module.merge(EthApi::new(data.clone()).into_rpc())?;
                },
                config::RPCModules::Net => {
                    final_module.merge(NetApi::new(data.clone()).into_rpc())?;
                },
                config::RPCModules::Flk => {
                    final_module.merge(FleekApi::new(data.clone()).into_rpc())?;
                },
            }
        }

        Ok(final_module)
    }
}

#[async_trait::async_trait]
impl<C: Collection> WithStartAndShutdown for Rpc<C> {
    async fn start(&self) {
        let server = self
            .build_server_from_config()
            .await
            .expect("RPC Server to build");

        let handle = server.start(self.module.clone());

        *self.handle.lock().await = Some(handle);
    }

    async fn shutdown(&self) {
        if let Some(handle) = std::mem::take(self.handle.lock().await.deref_mut()) {
            match handle.stop() {
                Ok(_) => (),
                Err(_) => return, // already stopped
            };

            handle.stopped().await;
        }
    }

    fn is_running(&self) -> bool {
        // Handle is removed from self when we shutdown server
        self.handle.blocking_lock().is_some()
    }
}

impl<C: Collection> RpcInterface<C> for Rpc<C> {
    fn init(
        config: Self::Config,
        mempool: MempoolSocket,
        query_runner: c!(C::ApplicationInterface::SyncExecutor),
        blockstore: C::BlockStoreInterface,
        fetcher: &C::FetcherInterface,
        archive_socket: Option<ArchiveSocket>,
    ) -> anyhow::Result<Self> {
        let data: Arc<Data<C>> = Arc::new(Data {
            query_runner,
            mempool_socket: mempool,
            fetcher_socket: fetcher.get_socket(),
            _blockstore: blockstore,
            archive_socket,
        });

        let module = Self::create_modules_from_config(&config, data)?;

        Ok(Self {
            config,
            module,
            handle: Mutex::new(None),
            phantom: PhantomData,
        })
    }
}

impl<C: Collection> ConfigConsumer for Rpc<C> {
    type Config = crate::config::Config;

    const KEY: &'static str = "rpc";
}
