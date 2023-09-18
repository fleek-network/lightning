use std::fs::remove_file;
use std::sync::Arc;

use anyhow::{anyhow, Result};
use lightning_interfaces::config::ConfigProviderInterface;
use lightning_interfaces::infu_collection::{Collection, Node};
use lightning_interfaces::signer::SignerInterface;
use lightning_signer::Signer;
use resolved_pathbuf::ResolvedPathBuf;

use crate::args::KeySubCmd;
use crate::config::TomlConfigProvider;
use crate::node::FinalTypes;

pub async fn exec(key: KeySubCmd, config_path: ResolvedPathBuf) -> Result<()> {
    match key {
        KeySubCmd::Show => show_key().await,
        KeySubCmd::Generate => generate_key::<FinalTypes>(config_path).await,
    }
}

async fn generate_key<C: Collection<SignerInterface = Signer<C>>>(
    config_path: ResolvedPathBuf,
) -> Result<()> {
    let config = Arc::new(load_or_write_config::<C>(config_path).await?);
    let signer_config = config.get::<C::SignerInterface>();

    if signer_config.node_key_path.exists() {
        return Err(anyhow!(
            "Node secret key exists at specified path. Not generating keys."
        ));
    }

    if signer_config.consensus_key_path.exists() {
        return Err(anyhow!(
            "Consensus secret key exists at specified path. Not generating keys."
        ));
    }

    match Signer::<C>::generate_node_key(&signer_config.node_key_path) {
        Ok(_) => println!(
            "Successfully created node secret key at: {:?}",
            signer_config.node_key_path
        ),
        Err(err) => return Err(anyhow!("Failed to create node secret key: {err:?}")),
    };

    match Signer::<C>::generate_consensus_key(&signer_config.consensus_key_path) {
        Ok(_) => println!(
            "Successfully created consensus secret key at: {:?}",
            signer_config.consensus_key_path
        ),
        Err(err) => {
            remove_file(signer_config.node_key_path)?;
            return Err(anyhow!("Failed to create consensus secret key: {err:?}"));
        },
    };
    Ok(())
}

async fn show_key() -> Result<()> {
    Ok(())
}

async fn load_or_write_config<C: Collection>(
    config_path: ResolvedPathBuf,
) -> Result<TomlConfigProvider<C>> {
    let config = TomlConfigProvider::open(&config_path)?;
    Node::<C>::fill_configuration(&config);

    if !config_path.exists() {
        std::fs::write(&config_path, config.serialize_config())?;
    }

    Ok(config)
}
