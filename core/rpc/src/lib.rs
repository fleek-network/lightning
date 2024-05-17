use std::sync::Arc;

use fleek_crypto::{ConsensusPublicKey, NodePublicKey};
use hyper::server::conn::AddrStream;
use hyper::service::{make_service_fn, service_fn};
use jsonrpsee::server::{stop_channel, Server as JSONRPCServer};
use jsonrpsee::{Methods, RpcModule};
use lightning_interfaces::prelude::*;
use lightning_interfaces::types::Event;
use lightning_interfaces::{FetcherSocket, MempoolSocket};
use reqwest::StatusCode;
use tower::Service;

use crate::api::AdminApiServer;
pub use crate::api::{EthApiServer, FleekApiServer, NetApiServer};
pub use crate::config::Config;
use crate::error::RPCError;
use crate::event::EventDistributor;
use crate::logic::AdminApi;
pub use crate::logic::{EthApi, FleekApi, NetApi};

pub mod api;
pub mod api_types;
pub mod config;
pub mod error;
pub mod event;
mod logic;

#[cfg(test)]
mod tests;

pub(crate) struct Data<C: Collection> {
    pub query_runner: c!(C::ApplicationInterface::SyncExecutor),
    pub mempool_socket: MempoolSocket,
    pub fetcher_socket: FetcherSocket,
    pub _blockstore: C::BlockstoreInterface,
    pub node_public_key: NodePublicKey,
    pub consensus_public_key: ConsensusPublicKey,
    pub archive: C::ArchiveInterface,
    pub event_handler: EventDistributor,
}

impl<C: Collection> Data<C> {
    pub(crate) async fn query_runner(
        &self,
        epoch: Option<u64>,
    ) -> Result<c!(C::ApplicationInterface::SyncExecutor), RPCError> {
        if let Some(epoch) = epoch {
            if let Some(query_runner) = self.archive.get_historical_epoch_state(epoch).await {
                Ok(query_runner)
            } else {
                Err(RPCError::BadEpoch)
            }
        } else {
            Ok(self.query_runner.clone())
        }
    }
}

pub struct Rpc<C: Collection> {
    config: Config,
    /// The final RPCModule containting selected methods
    module: RpcModule<()>,
    /// RPC module for admin methods.
    admin_module: RpcModule<()>,
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
    /// Initialize the RPC-server, with the given parameters.
    fn init(
        config: &C::ConfigProviderInterface,
        forwarder: &C::ForwarderInterface,
        blockstore: &C::BlockstoreInterface,
        fetcher: &C::FetcherInterface,
        keystore: &C::KeystoreInterface,
        fdi::Cloned(archive): fdi::Cloned<c!(C::ArchiveInterface)>,
        fdi::Cloned(query_runner): fdi::Cloned<c!(C::ApplicationInterface::SyncExecutor)>,
    ) -> anyhow::Result<Self> {
        let config = config.get::<Self>();
        let data: Arc<Data<C>> = Arc::new(Data {
            query_runner,
            mempool_socket: forwarder.mempool_socket(),
            fetcher_socket: fetcher.get_socket(),
            _blockstore: blockstore.clone(),
            node_public_key: keystore.get_ed25519_pk(),
            consensus_public_key: keystore.get_bls_pk(),
            archive,
            event_handler: EventDistributor::spawn(),
        });
        let module = Self::create_modules_from_config(&config, data.clone())?;
        let admin_module = Self::create_admin_module_from_config(&config, data.clone())?;

        Ok(Self {
            config,
            module,
            admin_module,
            data,
        })
    }

    fn start(&self, shutdown: fdi::Cloned<ShutdownWaiter>) {
        let (stop, server_handle) = stop_channel();
        let json_rpc_service = JSONRPCServer::builder()
            .to_service_builder()
            .build(Methods::from(self.module.clone()), stop.clone());

        let admin_json_rpc_service = JSONRPCServer::builder()
            .to_service_builder()
            .build(Methods::from(self.admin_module.clone()), stop.clone());

        let make_service = make_service_fn(move |addr_stream: &AddrStream| {
            let json_rpc_service = json_rpc_service.clone();
            let admin_json_rpc_service = admin_json_rpc_service.clone();

            // Although it is kind of odd to set up a server in this manner,
            // this is safe from spoofing attacks as long as we're using TCP.
            // It would be better to add auth to these endpoints or add
            // another server on loopback for internal services.
            // Todo: improve admin service.
            let is_local = addr_stream.remote_addr().ip().is_loopback();

            async move {
                Ok::<_, hyper::Error>(service_fn(move |req: hyper::Request<hyper::Body>| {
                    let mut json_rpc_service = json_rpc_service.clone();
                    let mut admin_json_rpc_service = admin_json_rpc_service.clone();

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
                            "/admin" => {
                                if is_local && method == hyper::Method::POST {
                                    match admin_json_rpc_service.call(req).await {
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
                            _ => match json_rpc_service.call(req).await {
                                Ok(res) => Ok(res),
                                Err(err) => hyper::Response::builder()
                                    .status(StatusCode::INTERNAL_SERVER_ERROR)
                                    .body(hyper::Body::from(err.to_string())),
                            },
                        }
                    }
                }))
            }
        });

        let addr = self.config.addr();
        let server = hyper::Server::bind(&addr).serve(make_service);

        tokio::spawn(async move {
            let graceful = server.with_graceful_shutdown(async move { stop.shutdown().await });
            graceful.await.expect("Rpc Server to start");
        });

        tokio::spawn(async move {
            shutdown.wait_for_shutdown().await;
            server_handle.stop().unwrap();
            server_handle.stopped().await;
        });
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
                config::RPCModules::Admin => {
                    // Admin RPC is private and it's set up separately than the public RPCs.
                },
            }
        }

        Ok(final_module)
    }

    fn create_admin_module_from_config(
        config: &Config,
        data: Arc<Data<C>>,
    ) -> anyhow::Result<RpcModule<()>> {
        let mut final_module = RpcModule::new(());
        if config
            .rpc_selection()
            .any(|module| matches!(module, config::RPCModules::Admin))
        {
            final_module.merge(AdminApi::new(data.clone()).into_rpc())?;
        }
        Ok(final_module)
    }
}

impl<C: Collection> RpcInterface<C> for Rpc<C> {
    fn event_tx(&self) -> tokio::sync::mpsc::Sender<Vec<Event>> {
        self.data.event_handler.sender()
    }
}

impl<C: Collection> ConfigConsumer for Rpc<C> {
    type Config = crate::config::Config;

    const KEY: &'static str = "rpc";
}

impl<C: Collection> fdi::BuildGraph for Rpc<C> {
    fn build_graph() -> fdi::DependencyGraph {
        fdi::DependencyGraph::default().with(Self::init.with_event_handler("start", Self::start))
    }
}
