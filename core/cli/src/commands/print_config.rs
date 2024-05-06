use anyhow::Result;
use lightning_interfaces::prelude::*;
use lightning_utils::config::TomlConfigProvider;
use resolved_pathbuf::ResolvedPathBuf;

pub async fn exec<C: Collection>(default: bool, config_path: ResolvedPathBuf) -> Result<()> {
    match default {
        true => print_default::<C>().await,
        false => print::<C>(config_path).await,
    }
}

async fn print_default<C: Collection>() -> Result<()> {
    let config = TomlConfigProvider::<C>::default();
    <C as Collection>::capture_configs(&config);
    println!("{}", config.serialize_config());
    Ok(())
}

async fn print<C: Collection>(config_path: ResolvedPathBuf) -> Result<()> {
    let config = TomlConfigProvider::<C>::load_or_write_config(config_path).await?;
    println!("{}", config.serialize_config());
    Ok(())
}
