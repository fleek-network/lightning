use std::net::SocketAddr;

use anyhow::{anyhow, Result};
use lightning_application::app::Application;
use lightning_application::config::Config as AppConfig;
use lightning_application::network::Network;
use lightning_final_bindings::FinalTypes;
use lightning_handshake::config::HandshakeConfig;
use lightning_handshake::handshake::Handshake;
use lightning_interfaces::prelude::*;
use lightning_rpc::{Config as RpcConfig, Rpc};
use lightning_utils::config::TomlConfigProvider;
use resolved_pathbuf::ResolvedPathBuf;
use tracing::info;

pub async fn exec<C>(
    config_path: ResolvedPathBuf,
    network: Option<Network>,
    no_generate_keys: bool,
    dev: bool,
    rpc_address: Option<SocketAddr>,
    handshake_http_address: Option<SocketAddr>,
) -> Result<()>
where
    C: Collection<ConfigProviderInterface = TomlConfigProvider<C>>,
{
    // Error if the configuration file already exists.
    if config_path.exists() {
        return Err(anyhow!(
            "Configuration file already exists at {}",
            config_path.to_str().unwrap()
        ));
    }

    // Initialize a new configuration with defaults.
    let config = TomlConfigProvider::<C>::new();
    <C as Collection>::capture_configs(&config);

    // Validate that either network or dev are set, and assign network if dev is set.
    if !dev && network.is_none() {
        return Err(anyhow!(
            "Either --network or --dev must be provided. See --help for details."
        ));
    }

    // Set network field in the configuration.
    let mut app_config = AppConfig::default();
    if dev {
        app_config.network = Some(Network::LocalnetExample);
        app_config.dev = Some(Default::default());
    } else {
        app_config.network = network;
    }
    config.inject::<Application<FinalTypes>>(app_config);

    // Update RPC address in the configuration if given.
    if let Some(addr) = rpc_address {
        config.inject::<Rpc<FinalTypes>>(RpcConfig {
            addr,
            ..Default::default()
        });
    }

    // Update handshake HTTP address in the configuration if given.
    if let Some(addr) = handshake_http_address {
        config.inject::<Handshake<FinalTypes>>(HandshakeConfig {
            http_address: addr,
            ..Default::default()
        });
    }

    // Write the configuration file.
    config.write(&config_path)?;
    info!(
        "Configuration file written to {}",
        config_path.to_str().unwrap()
    );

    // Generate keys if requested.
    if !no_generate_keys {
        let keystore_config = config.get::<C::KeystoreInterface>();
        C::KeystoreInterface::generate_keys(keystore_config, true)?;
    }

    Ok(())
}
