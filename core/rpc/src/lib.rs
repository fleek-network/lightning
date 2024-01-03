use std::ops::DerefMut;
use std::sync::Arc;

use fleek_crypto::{ConsensusPublicKey, NodePublicKey};
use hyper::service::{make_service_fn, service_fn};
use jsonrpsee::server::{stop_channel, Server as JSONRPCServer, ServerHandle};
use jsonrpsee::{Methods, RpcModule};
use lightning_interfaces::infu_collection::{c, Collection};
use lightning_interfaces::types::Event;
use lightning_interfaces::{
    ApplicationInterface,
    ArchiveSocket,
    ConfigConsumer,
    FetcherInterface,
    FetcherSocket,
    MempoolSocket,
    RpcInterface,
    SignerInterface,
    WithStartAndShutdown,
};
use reqwest::StatusCode;
use tokio::sync::Mutex;
use tower::Service;

pub use crate::api::{EthApiServer, FleekApiServer, NetApiServer};
pub use crate::config::Config;
use crate::event::EventDistributor;
pub use crate::logic::{EthApi, FleekApi, NetApi};
pub mod api;
pub mod api_types;
pub mod config;
pub mod error;
pub mod event;
pub mod logic;
pub mod utils;

#[cfg(test)]
mod tests;

pub(crate) struct Data<C: Collection> {
    pub event_distributor: EventDistributor,
    pub query_runner: c!(C::ApplicationInterface::SyncExecutor),
    pub mempool_socket: MempoolSocket,
    pub fetcher_socket: FetcherSocket,
    pub _blockstore: C::BlockStoreInterface,
    pub node_public_key: NodePublicKey,
    pub consensus_public_key: ConsensusPublicKey,
    /// If this is some it means the node is in archive mode
    pub archive_socket: Option<ArchiveSocket>,
}

pub struct Rpc<C: Collection> {
    config: Config,

    /// The final RPCModule containting selected methods
    module: RpcModule<()>,

    // need interior mutability to support restarts
    handle: Mutex<Option<ServerHandle>>,

    data: Arc<Data<C>>,
}

async fn health() -> &'static str {
    "OK"
}

async fn metrics() -> (StatusCode, String) {
    match autometrics::prometheus_exporter::encode_to_string() {
        Ok(metrics) => (StatusCode::OK, metrics),
        Err(err) => (StatusCode::INTERNAL_SERVER_ERROR, err.to_string()),
    }
}

impl<C: Collection> Rpc<C> {
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
        let (stop, server_handle) = stop_channel();
        let json_rpc_service = JSONRPCServer::builder()
            .to_service_builder()
            .build(Methods::from(self.module.clone()), stop.clone());

        let make_service = make_service_fn(move |_| {
            let json_rpc_service = json_rpc_service.clone();

            async move {
                Ok::<_, hyper::Error>(service_fn(move |req: hyper::Request<hyper::Body>| {
                    let mut json_rpc_service = json_rpc_service.clone();

                    async move {
                        let path = req.uri().path().to_string().to_ascii_lowercase();
                        let method = req.method();

                        match path.as_str() {
                            "/health" => {
                                let res = health().await;

                                hyper::Response::builder()
                                    .status(StatusCode::OK)
                                    .body(hyper::Body::from(res))
                            },
                            "/metrics" => {
                                let (status, res) = metrics().await;

                                hyper::Response::builder()
                                    .status(status)
                                    .body(hyper::Body::from(res))
                            },
                            _ => {
                                if method == hyper::Method::POST {
                                    match json_rpc_service.call(req).await {
                                        Ok(res) => Ok(res),
                                        Err(err) => hyper::Response::builder()
                                            .status(StatusCode::INTERNAL_SERVER_ERROR)
                                            .body(hyper::Body::from(err.to_string())),
                                    }
                                } else {
                                    hyper::Response::builder()
                                        .status(StatusCode::NOT_FOUND)
                                        .body(hyper::Body::empty())
                                }
                            },
                        }
                    }
                }))
            }
        });

        let addr = self.config.addr();
        tokio::spawn(async move {
            match axum::Server::bind(&addr)
                .serve(make_service)
                .with_graceful_shutdown(async move { stop.shutdown().await })
                .await
            {
                Ok(_) => (),
                Err(err) => tracing::error!("RPC server error: {}", err),
            }
        });

        *self.handle.lock().await = Some(server_handle);
    }

    async fn shutdown(&self) {
        if let Some(handle) = std::mem::take(self.handle.lock().await.deref_mut()) {
            match handle.stop() {
                Ok(_) => (),
                Err(_) => return,
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
        signer: &C::SignerInterface,
        archive_socket: Option<ArchiveSocket>,
    ) -> anyhow::Result<Self> {
        let distributor = EventDistributor::spawn();

        let data: Arc<Data<C>> = Arc::new(Data {
            event_distributor: distributor,
            query_runner,
            mempool_socket: mempool,
            fetcher_socket: fetcher.get_socket(),
            _blockstore: blockstore,
            node_public_key: signer.get_ed25519_pk(),
            consensus_public_key: signer.get_bls_pk(),
            archive_socket,
        });

        let module = Self::create_modules_from_config(&config, data.clone())?;

        Ok(Self {
            config,
            module,
            data,
            handle: Mutex::new(None),
        })
    }

    fn event_tx(&self) -> tokio::sync::mpsc::Sender<Vec<Event>> {
        self.data.event_distributor.sender()
    }
}

impl<C: Collection> ConfigConsumer for Rpc<C> {
    type Config = crate::config::Config;

    const KEY: &'static str = "rpc";
}
