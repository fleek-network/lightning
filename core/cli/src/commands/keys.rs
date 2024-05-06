use anyhow::Result;
use lightning_interfaces::prelude::*;
use lightning_utils::config::TomlConfigProvider;
use resolved_pathbuf::ResolvedPathBuf;

use crate::args::KeySubCmd;

pub async fn exec<C: Collection>(cmd: KeySubCmd, config_path: ResolvedPathBuf) -> Result<()> {
    let provider = TomlConfigProvider::<C>::load_or_write_config(config_path).await?;
    let config = provider.get::<C::KeystoreInterface>();

    match cmd {
        KeySubCmd::Generate => C::KeystoreInterface::generate_keys(config, false),
        // KeySubCmd::Show => C::KeystoreInterface::init(config).map(|_| ()),
        _ => todo!(),
    }
}
