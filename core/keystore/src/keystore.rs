use std::fs::{self, create_dir_all, read_to_string, File};
use std::io::Write;
use std::marker::PhantomData;
use std::ops::Deref;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;

use anyhow::{bail, Context};
use fleek_crypto::{
    ConsensusPublicKey,
    ConsensusSecretKey,
    NodePublicKey,
    NodeSecretKey,
    SecretKey,
};
use lightning_interfaces::infu_collection::Collection;
use lightning_interfaces::{ConfigConsumer, KeystoreInterface};
use triomphe::Arc;

use crate::KeystoreConfig;

#[derive(Clone)]
pub struct Keystore<C> {
    node_secret_key: Arc<NodeSecretKey>,
    consensus_secret_key: Arc<ConsensusSecretKey>,
    _p: PhantomData<C>,
}

impl<C> ConfigConsumer for Keystore<C> {
    const KEY: &'static str = "keystore";
    type Config = KeystoreConfig;
}

impl<C: Collection> KeystoreInterface<C> for Keystore<C> {
    fn init(config: Self::Config) -> anyhow::Result<Self> {
        let node_secret_key = if config.node_key_path.exists() {
            let encoded =
                read_to_string(&config.node_key_path).context("Failed to read node pem file")?;
            NodeSecretKey::decode_pem(&encoded).context("Failed to decode node pem file")?
        } else {
            bail!("Node secret key does not exist. Use the CLI to generate keys.");
        }
        .into();

        let consensus_secret_key = if config.consensus_key_path.exists() {
            let encoded = read_to_string(&config.consensus_key_path)
                .context("Failed to read consensus pem file")?;
            ConsensusSecretKey::decode_pem(&encoded)
                .context("Failed to decode consensus pem file")?
        } else {
            bail!("Consensus secret key does not exist. Use the CLI to generate keys.");
        }
        .into();

        Ok(Self {
            node_secret_key,
            consensus_secret_key,
            _p: PhantomData,
        })
    }

    fn get_ed25519_pk(&self) -> NodePublicKey {
        self.node_secret_key.to_pk()
    }

    fn get_ed25519_sk(&self) -> NodeSecretKey {
        self.node_secret_key.deref().clone()
    }

    fn get_bls_pk(&self) -> ConsensusPublicKey {
        self.consensus_secret_key.to_pk()
    }

    fn get_bls_sk(&self) -> ConsensusSecretKey {
        self.consensus_secret_key.deref().clone()
    }

    fn generate_keys(config: Self::Config, accept_partial: bool) -> anyhow::Result<()> {
        let (node_exists, consensus_exists) = (
            config.node_key_path.exists(),
            config.consensus_key_path.exists(),
        );

        if !accept_partial {
            if node_exists {
                bail!(
                    "Cannot overwrite existing ed25519 key {:?}",
                    config.node_key_path
                );
            }
            if consensus_exists {
                bail!(
                    "Cannot overwrite existing bls consensus key {:?}",
                    config.consensus_key_path
                );
            }
        }

        if !node_exists {
            let node_secret_key = NodeSecretKey::generate();
            save(&config.node_key_path, node_secret_key.encode_pem())?;
        }
        if !consensus_exists {
            let consensus_secret_key = ConsensusSecretKey::generate();
            save(
                &config.consensus_key_path,
                consensus_secret_key.encode_pem(),
            )?;
        }

        Ok(())
    }
}

fn save<T: AsRef<[u8]>>(path: &Path, data: T) -> anyhow::Result<()> {
    create_dir_all(path.parent().unwrap())?;
    let mut file = File::create(path)?;
    file.write_all(data.as_ref())?;
    file.sync_all()?;
    let mut perms = file.metadata()?.permissions();
    perms.set_mode(0o600);
    fs::set_permissions(path, perms)?;
    Ok(())
}
