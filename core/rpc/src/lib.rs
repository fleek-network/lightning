use std::sync::Arc;

use fleek_crypto::{ConsensusPublicKey, NodePublicKey};
use jsonrpsee::server::{stop_channel, Server as JSONRPCServer};
use jsonrpsee::{Methods, RpcModule};
use lightning_firewall::Firewall;
use lightning_interfaces::prelude::*;
use lightning_interfaces::{Events, FetcherSocket, MempoolSocket};
use reqwest::StatusCode;
use tokio::sync::OnceCell;

use crate::api::AdminApiServer;
pub use crate::api::{EthApiServer, FleekApiServer, NetApiServer};
pub use crate::config::Config;
use crate::error::RPCError;
use crate::logic::AdminApi;
pub use crate::logic::{EthApi, FleekApi, NetApi};

pub mod api;
pub mod api_types;
pub mod config;
pub mod error;
mod logic;

mod server;
#[cfg(test)]
mod tests;

/// Cache the config so we dont need to clone per connection
pub static CONFIG: OnceCell<config::Config> = OnceCell::const_new();

/// The data shared with every request the rpc methods.
pub(crate) struct Data<C: Collection> {
    pub query_runner: c!(C::ApplicationInterface::SyncExecutor),
    pub mempool_socket: MempoolSocket,
    pub fetcher_socket: FetcherSocket,
    pub _blockstore: C::BlockstoreInterface,
    pub node_public_key: NodePublicKey,
    pub consensus_public_key: ConsensusPublicKey,
    pub archive: C::ArchiveInterface,
    pub events: Events,
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

pub async fn health() -> &'static str {
    "OK"
}

pub async fn metrics() -> (StatusCode, String) {
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
        // this should only get ran once
        let _ = CONFIG.set(config.clone());

        let data: Arc<Data<C>> = Arc::new(Data {
            query_runner,
            mempool_socket: forwarder.mempool_socket(),
            fetcher_socket: fetcher.get_socket(),
            _blockstore: blockstore.clone(),
            node_public_key: keystore.get_ed25519_pk(),
            consensus_public_key: keystore.get_bls_pk(),
            archive,
            events: {
                let (tx, _) = tokio::sync::broadcast::channel(8);
                tx.into()
            },
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

        let disallowed = self.config.disallowed_methods.as_ref().map(|s| s.as_ref());
        let json_rpc_service = JSONRPCServer::builder().to_service_builder().build(
            filter_methods(self.module.clone(), disallowed),
            stop.clone(),
        );

        let admin_json_rpc_service = JSONRPCServer::builder().to_service_builder().build(
            filter_methods(self.admin_module.clone(), disallowed),
            stop.clone(),
        );

        let rpc_server = server::RpcService::new(json_rpc_service, admin_json_rpc_service);
        let rpc_server =
            Firewall::new("rpc", Default::default(), Default::default()).service(rpc_server);

        let addr = self.config.addr();
        let server = hyper::Server::bind(&addr).serve(rpc_server);

        let panic_waiter = shutdown.clone();
        spawn!(
            async move {
                let graceful = server.with_graceful_shutdown(async move { stop.shutdown().await });
                graceful.await.expect("Rpc Server to start");
            },
            "RPC",
            crucial(panic_waiter)
        );

        spawn!(
            async move {
                shutdown.wait_for_shutdown().await;
                server_handle.stop().unwrap();
                server_handle.stopped().await;
            },
            "RPC: shutdown waiter"
        );
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
    fn event_tx(&self) -> Events {
        self.data.events.clone()
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

/// Panics if any of the disallowed methods are not valid methods.
fn filter_methods(
    into_methods: impl Into<Methods>,
    disallowed: Option<impl AsRef<[String]>>,
) -> Methods {
    let methods: Methods = into_methods.into();
    if disallowed.is_none() {
        return methods;
    }

    let disallowed = disallowed.unwrap();

    let mut filtered = Methods::new();
    let method_names: Vec<&_> = methods.method_names().collect();

    // Check if the disallowed methods are valid methods
    for method in disallowed.as_ref() {
        if !method_names.contains(&method.as_ref()) {
            panic!(
                "Disallowed method {} is not a valid method, check your config settings",
                method
            );
        }
    }

    // Check if the methods are disallowed
    let disallowed: Vec<_> = disallowed.as_ref().iter().map(|s| s.as_str()).collect();
    for method in method_names {
        if !disallowed.contains(&method) {
            // this should work because we know the method is valid already and filtered
            // is emtpy
            let _ = filtered.verify_and_insert(method, methods.method(method).unwrap().clone());
        }
    }

    filtered
}
