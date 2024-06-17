use std::fs::read_to_string;
use std::path::PathBuf;
use std::sync::atomic::AtomicUsize;
use std::sync::{Arc, OnceLock};

use anyhow::Context;
use fleek_crypto::{ConsensusPublicKey, NodePublicKey};
use jsonrpsee::server::{stop_channel, Server as JSONRPCServer};
use jsonrpsee::{Methods, RpcModule};
use lightning_firewall::Firewall;
use lightning_interfaces::prelude::*;
use lightning_interfaces::{Events, FetcherSocket, MempoolSocket};
use lightning_utils::config::LIGHTNING_HOME_DIR;
use rand::{RngCore, SeedableRng};
use reqwest::StatusCode;
use resolved_pathbuf::ResolvedPathBuf;

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

static HMAC_SECRET: OnceLock<[u8; 32]> = OnceLock::new();
pub static HMAC_NONCE: AtomicUsize = AtomicUsize::new(0);
pub static HMAC_SALT: &[u8] = b"lightning-hmac-salt";

/// Tries to read the hmac secret from the given path or the default FS location
/// or it will generate one and write it to disk
pub fn hmac_secret(secret_dir_path: Option<PathBuf>) -> anyhow::Result<&'static [u8; 32]> {
    match HMAC_SECRET.get() {
        Some(secret) => Ok(secret),
        None => {
            let path = secret_dir_path.unwrap_or_else(|| LIGHTNING_HOME_DIR.to_path_buf());

            std::fs::create_dir_all(&path).context("Failed to create_dir_all path")?;
            let parent = ResolvedPathBuf::try_from(path)?;
            let secret_path = parent.join("hmac_secret.hex");

            let secret_bytes = if secret_path.is_file() {
                tracing::info!("Reading HMAC secret from file");

                let secret_hex = read_to_string(&secret_path)?;
                let secret_bytes = hex::decode(secret_hex.trim())?;
                assert!(
                    secret_bytes.len() == 32,
                    "HMAC secret must be hex encoded and 32 bytes"
                );

                let mut secret = [0_u8; 32];
                secret.copy_from_slice(&secret_bytes);

                secret
            } else {
                tracing::info!("Generating new HMAC secret");

                let mut dest = [0u8; 32];
                rand::rngs::StdRng::from_entropy().fill_bytes(&mut dest);

                let secret_hex = hex::encode(dest);
                std::fs::File::create(&secret_path)?;
                std::fs::write(&secret_path, secret_hex)?;

                dest
            };

            HMAC_SECRET.set(secret_bytes).unwrap();
            Ok(HMAC_SECRET.get().unwrap())
        },
    }
}

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
    firewall: Firewall,
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
        config_provider: &C::ConfigProviderInterface,
        forwarder: &C::ForwarderInterface,
        blockstore: &C::BlockstoreInterface,
        fetcher: &C::FetcherInterface,
        keystore: &C::KeystoreInterface,
        fdi::Cloned(archive): fdi::Cloned<c!(C::ArchiveInterface)>,
        fdi::Cloned(query_runner): fdi::Cloned<c!(C::ApplicationInterface::SyncExecutor)>,
    ) -> anyhow::Result<Self> {
        let mut config = config_provider.get::<Self>();

        let _ = hmac_secret(config.hmac_secret_dir.take())?;

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
            firewall: config_provider.firewall::<Self>(),
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
        let rpc_server = self.firewall.clone().service(rpc_server);

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
