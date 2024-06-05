use anyhow::{anyhow, Context, Result};
use lightning_application::app::Application;
use lightning_application::config::Config as AppConfig;
use lightning_application::network::Network;
use lightning_final_bindings::FinalTypes;
use lightning_interfaces::prelude::*;
use lightning_utils::config::TomlConfigProvider;
use resolved_pathbuf::ResolvedPathBuf;
use tracing::info;

pub async fn exec<C>(
    config_path: ResolvedPathBuf,
    network: Network,
    no_generate_keys: bool,
    no_apply_genesis: bool,
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

    // Set network field in the configuration.
    config.inject::<Application<FinalTypes>>(AppConfig {
        network: Some(network),
        ..Default::default()
    });

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

    // Execute genesis if requested.
    if no_generate_keys {
        info!(
            "Not loading genesis block, since keys were not generated. It will be loaded when starting the node after you generate or import keys."
        );
    } else if !no_apply_genesis {
        Node::<C>::init(config).context("Failed to execute genesis")?;
    }

    Ok(())
}
