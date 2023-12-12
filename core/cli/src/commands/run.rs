use std::future::Future;
use std::pin::Pin;
use std::time::Duration;

use anyhow::{Context, Result};
use infusion::tag;
use lightning_interfaces::infu_collection::{Collection, Node};
use lightning_interfaces::{
    ApplicationInterface,
    BlockStoreInterface,
    ConfigProviderInterface,
    SyncronizerInterface,
};
use lightning_node::config::TomlConfigProvider;
use resolved_pathbuf::ResolvedPathBuf;
use tokio::pin;
use tracing::warn;

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
    let app_config = config.get::<<C as Collection>::ApplicationInterface>();

    let mut node = Node::<C>::init(config.clone())
        .map_err(|e| anyhow::anyhow!("Node Initialization failed: {e:?}"))
        .context("Could not start the node.")?;

    if let Some(cb) = &custom_start_shutdown {
        ((cb)(&node, true)).await;
    } else {
        node.start().await;
    }

    let mut rx_update_ready = node
        .container
        .get::<<C as Collection>::SyncronizerInterface>(tag!(C::SyncronizerInterface))
        .checkpoint_socket();

    let shutdown_future = shutdown_controller.wait_for_shutdown();
    pin!(shutdown_future);

    loop {
        tokio::select! {
            _ = &mut shutdown_future => break,
            Ok(checkpoint_hash) = &mut rx_update_ready => {
                // get the checkpoint from the blockstore
                let checkpoint = node
                .container
                .get::<<C as Collection>::BlockStoreInterface>(tag!(C::BlockStoreInterface))
                .read_all_to_vec(&checkpoint_hash).await.expect("Failed to read checkpoint from blockstore");

                // shutdown the node
                if let Some(cb) = &custom_start_shutdown {
                    ((cb)(&node, false)).await;
                } else {
                    node.shutdown().await;
                }

                std::mem::drop(node);

                // Sleep for a bit but provide some feedback, some of our proccesses take a few milliseconds to drop from memory
                warn!("Preparing to load checkpoint, Restarting services");
                tokio::time::sleep(Duration::from_secs(3)).await;
                // start local env in checkpoint mode to seed database with the new checkpoint
                C::ApplicationInterface::load_from_checkpoint(
                    &app_config, checkpoint, checkpoint_hash)?;

                node = Node::<C>::init(config.clone())
                    .map_err(|e| anyhow::anyhow!("Could not start the node: {e:?}"))?;

                //restart the node
                if let Some(cb) = &custom_start_shutdown {
                    ((cb)(&node, true)).await;
                } else {
                    node.start().await;
                }

                // reseed our rx_update_ready
                rx_update_ready = node
                .container
                .get::<<C as Collection>::SyncronizerInterface>(tag!(C::SyncronizerInterface))
                .checkpoint_socket();
            }
        }
    }

    if let Some(cb) = custom_start_shutdown {
        ((cb)(&node, false)).await;
    } else {
        node.shutdown().await;
    }

    Ok(())
}
