use std::future::Future;
use std::pin::Pin;

use anyhow::{Context, Result};
use lightning_interfaces::infu_collection::{Collection, Node};
use lightning_node::config::TomlConfigProvider;
use resolved_pathbuf::ResolvedPathBuf;

use crate::shutdown::ShutdownController;

pub type CustomStartShutdown<C> = Box<dyn for<'a> Fn(&'a Node<C>, bool) -> Fut<'a>>;
pub type Fut<'a> = Pin<Box<dyn Future<Output = ()> + 'a>>;
pub async fn exec<C>(
    config_path: ResolvedPathBuf,
    custom_start_shutdown: Option<CustomStartShutdown<C>>,
) -> Result<()>
where
    C: Collection<ConfigProviderInterface = TomlConfigProvider<C>>,
{
    let shutdown_controller = ShutdownController::default();
    shutdown_controller.install_handlers();

    let config = TomlConfigProvider::<C>::load_or_write_config(config_path).await?;

    let node = Node::<C>::init(config)
        .map_err(|e| anyhow::anyhow!("Node Initialization failed: {e:?}"))
        .context("Could not start the node.")?;

    if let Some(cb) = &custom_start_shutdown {
        ((cb)(&node, true)).await;
    } else {
        node.start().await;
    }

    shutdown_controller.wait_for_shutdown().await;

    if let Some(cb) = custom_start_shutdown {
        ((cb)(&node, false)).await;
    } else {
        node.shutdown().await;
    }

    Ok(())
}
