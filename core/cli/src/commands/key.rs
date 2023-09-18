use std::fs::{read_to_string, remove_file};
use std::sync::Arc;

use anyhow::{anyhow, Context, Result};
use fleek_crypto::{ConsensusSecretKey, NodeSecretKey, PublicKey, SecretKey};
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
        KeySubCmd::Show => show_key::<FinalTypes>(config_path).await,
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

async fn show_key<C: Collection<SignerInterface = Signer<C>>>(
    config_path: ResolvedPathBuf,
) -> Result<()> {
    let config = Arc::new(load_or_write_config::<C>(config_path).await?);
    let signer_config = config.get::<C::SignerInterface>();
    if signer_config.node_key_path.exists() {
        let node_secret_key = read_to_string(&signer_config.node_key_path)
            .with_context(|| "Failed to read node pem file")?;
        let node_secret_key = NodeSecretKey::decode_pem(&node_secret_key)
            .with_context(|| "Failed to decode node pem file")?;
        println!("Node Public Key: {}", node_secret_key.to_pk().to_base64());
    } else {
        eprintln!("Node Public Key: does not exist");
    }

    if signer_config.consensus_key_path.exists() {
        let consensus_secret_key = read_to_string(&signer_config.consensus_key_path)
            .with_context(|| "Failed to read consensus pem file")?;
        let consensus_secret_key = ConsensusSecretKey::decode_pem(&consensus_secret_key)
            .with_context(|| "Failed to decode consensus pem file")?;
        println!(
            "Consensus Public Key: {}",
            consensus_secret_key.to_pk().to_base64()
        );
    } else {
        eprintln!("Consensus Public Key: does not exist");
    }
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
