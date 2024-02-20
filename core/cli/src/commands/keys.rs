use anyhow::Result;
use lightning_interfaces::config::ConfigProviderInterface;
use lightning_interfaces::infu_collection::Collection;
use lightning_interfaces::KeystoreInterface;
use lightning_node::config::TomlConfigProvider;
use resolved_pathbuf::ResolvedPathBuf;

use crate::args::KeySubCmd;

pub async fn exec<C: Collection>(cmd: KeySubCmd, config_path: ResolvedPathBuf) -> Result<()> {
    let provider = TomlConfigProvider::<C>::load_or_write_config(config_path).await?;

    if cmd == KeySubCmd::Generate {
        C::KeystoreInterface::generate_keys(provider.get::<C::KeystoreInterface>(), false)?;
    }

    let keystore = C::KeystoreInterface::init(provider.get::<C::KeystoreInterface>())?;
    println!("Node Public Key: {}", keystore.get_ed25519_pk());
    println!("Consensus Public Key: {}", keystore.get_bls_pk());

    Ok(())
}
