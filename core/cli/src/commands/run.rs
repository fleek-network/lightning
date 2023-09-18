use std::future::Future;
use std::pin::Pin;

use anyhow::Result;
use lightning_application::app::Application;
use lightning_blockstore::blockstore::Blockstore;
use lightning_blockstore_server::BlockStoreServer;
use lightning_interfaces::infu_collection::{Collection, Node};
use lightning_interfaces::ConfigProviderInterface;
use lightning_signer::Signer;
use resolved_pathbuf::ResolvedPathBuf;

use crate::config::TomlConfigProvider;
use crate::shutdown::ShutdownController;
use crate::testnet_sync;

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

    // testnet sync
    let signer_config = config.get::<Signer<C>>();
    let app_config = config.get::<Application<C>>();
    let blockstore_config = config.get::<Blockstore<C>>();
    let block_server_config = config.get::<BlockStoreServer<C>>();
    if app_config.testnet {
        testnet_sync::sync(
            signer_config,
            app_config,
            blockstore_config,
            block_server_config,
        )
        .await;
    }

    let node =
        Node::<C>::init(config).map_err(|e| anyhow::anyhow!("Could not start the node: {e}"))?;

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
