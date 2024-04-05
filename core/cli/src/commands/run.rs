use std::time::Duration;

use anyhow::{Context, Result};
use lightning_interfaces::infu_collection::{Collection, Node};
use lightning_interfaces::{
    ApplicationInterface,
    BlockstoreInterface,
    ConfigProviderInterface,
    SyncronizerInterface,
};
use lightning_node::config::TomlConfigProvider;
use lightning_utils::shutdown::ShutdownController;
use resolved_pathbuf::ResolvedPathBuf;
use tokio::pin;
use tracing::warn;

pub async fn exec<C>(config_path: ResolvedPathBuf) -> Result<()>
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

    panic_report::add_context("config", config.into_inner());

    node.start().await;

    let mut checkpoint_fut = node
        .provider
        .get::<<C as Collection>::SyncronizerInterface>()
        .next_checkpoint_hash();

    let shutdown_future = shutdown_controller.wait_for_shutdown();
    pin!(shutdown_future);

    loop {
        tokio::select! {
            _ = &mut shutdown_future => break,
            checkpoint_hash = checkpoint_fut => {
                // get the checkpoint from the blockstore
                let checkpoint = node
                .provider
                .get::<<C as Collection>::BlockstoreInterface>()
                .read_all_to_vec(&checkpoint_hash).await.expect("Failed to read checkpoint from blockstore");

                // shutdown the node
                node.shutdown().await;
                std::mem::drop(node);

                // Sleep for a bit but provide some feedback, some of our proccesses take a few milliseconds to drop from memory
                warn!("Preparing to load checkpoint, Restarting services");
                tokio::time::sleep(Duration::from_secs(3)).await;

                // start local env in checkpoint mode to seed database with the new checkpoint
                C::ApplicationInterface::load_from_checkpoint(
                    &app_config, checkpoint, checkpoint_hash).await?;

                //restart the node
                node = Node::<C>::init(config.clone())
                    .map_err(|e| anyhow::anyhow!("Could not start the node: {e:?}"))?;
                node.start().await;

                // reseed our rx_update_ready
                checkpoint_fut = node
                .provider
                .get::<<C as Collection>::SyncronizerInterface>()
                .next_checkpoint_hash();
            }
        }
    }

    node.shutdown().await;

    Ok(())
}
