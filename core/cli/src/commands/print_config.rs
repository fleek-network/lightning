use anyhow::Result;
use lightning_interfaces::config::ConfigProviderInterface;
use lightning_interfaces::infu_collection::{Collection, Node};
use resolved_pathbuf::ResolvedPathBuf;

use crate::config::TomlConfigProvider;
use crate::node::FinalTypes;

pub async fn exec(default: bool, config_path: ResolvedPathBuf) -> Result<()> {
    match default {
        true => print_default::<FinalTypes>().await,
        false => print::<FinalTypes>(config_path).await,
    }
}

async fn print_default<C: Collection>() -> Result<()> {
    let config = TomlConfigProvider::<C>::default();
    Node::<C>::fill_configuration(&config);
    println!("{}", config.serialize_config());
    Ok(())
}

async fn print<C: Collection>(config_path: ResolvedPathBuf) -> Result<()> {
    let config = TomlConfigProvider::<C>::load_or_write_config(config_path).await?;
    println!("{}", config.serialize_config());
    Ok(())
}
