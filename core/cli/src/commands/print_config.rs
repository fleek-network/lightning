use anyhow::Result;
use lightning_interfaces::prelude::*;
use lightning_utils::config::TomlConfigProvider;
use resolved_pathbuf::ResolvedPathBuf;

pub async fn exec<C: NodeComponents>(default: bool, config_path: ResolvedPathBuf) -> Result<()> {
    match default {
        true => print_default::<C>().await,
        false => print::<C>(config_path).await,
    }
}

async fn print_default<C: NodeComponents>() -> Result<()> {
    let config = TomlConfigProvider::<C>::default();
    <C as NodeComponents>::capture_configs(&config);
    println!("{}", config.serialize_config());
    Ok(())
}

async fn print<C: NodeComponents>(config_path: ResolvedPathBuf) -> Result<()> {
    let config = TomlConfigProvider::<C>::load(config_path)?;
    println!("{}", config.serialize_config());
    Ok(())
}
