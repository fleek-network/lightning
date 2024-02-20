use anyhow::{Context, Result};
use lightning_interfaces::config::ConfigProviderInterface;
use lightning_interfaces::infu_collection::Collection;
use lightning_interfaces::KeystoreInterface;
use lightning_node::config::TomlConfigProvider;
use resolved_pathbuf::ResolvedPathBuf;

use crate::args::KeySubCmd;

pub async fn exec<C: Collection>(cmd: KeySubCmd, config_path: ResolvedPathBuf) -> Result<()> {
    let provider = TomlConfigProvider::<C>::load_or_write_config(config_path).await?;
    let config = provider.get::<C::KeystoreInterface>();

    match cmd {
        KeySubCmd::Generate => {
            C::KeystoreInterface::generate_keys(config, false).context("Could not generate key")
        },
        KeySubCmd::Show => {
            let keystore =
                C::KeystoreInterface::init(config).context("Failed to initialize keystore")?;

            println!("Node Public Key: {}", keystore.get_ed25519_pk());
            println!("Consensus Public Key: {}", keystore.get_bls_pk());

            Ok(())
        },
    }
}
