use std::time::Duration;

use anyhow::Result;
use lightning_interfaces::fdi::MultiThreadedProvider;
use lightning_interfaces::prelude::*;
use lightning_node::ContainedNode;
use lightning_utils::config::TomlConfigProvider;
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

    let config = TomlConfigProvider::<C>::load(config_path)?;
    let app_config = config.get::<<C as Collection>::ApplicationInterface>();

    let provider = MultiThreadedProvider::default();
    provider.insert(config.clone());
    let mut node = ContainedNode::<C>::new(provider, None);

    panic_report::add_context("config", config.into_inner());

    node.spawn().await??;

    let shutdown_future = shutdown_controller.wait_for_shutdown();
    pin!(shutdown_future);

    loop {
        let syncronizer = node
            .provider()
            .get::<<C as Collection>::SyncronizerInterface>();

        let checkpoint_fut = syncronizer.next_checkpoint_hash();

        tokio::select! {
            _ = &mut shutdown_future => break,
            Some(checkpoint_hash) = checkpoint_fut => {
                // get the checkpoint from the blockstore
                let checkpoint = node
                    .provider()
                    .get::<<C as Collection>::BlockstoreInterface>()
                    .read_all_to_vec(&checkpoint_hash).await.expect("Failed to read checkpoint from blockstore");

                // shutdown the node
                node.shutdown().await;

                // Sleep for a bit but provide some feedback, some of our proccesses take a few milliseconds to drop from memory
                warn!("Preparing to load checkpoint, Restarting services");
                tokio::time::sleep(Duration::from_secs(3)).await;

                // start local env in checkpoint mode to seed database with the new checkpoint
                C::ApplicationInterface::load_from_checkpoint(
                    &app_config,
                    checkpoint,
                    checkpoint_hash
                ).await?;

                let provider = MultiThreadedProvider::default();
                provider.insert(config.clone());
                node = ContainedNode::<C>::new(provider, None);
                node.spawn().await??;

            }
        }
    }

    node.shutdown().await;

    Ok(())
}
