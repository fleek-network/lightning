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
use lightning_metrics::increment_counter;
use lightning_node::config::TomlConfigProvider;
use lightning_utils::shutdown::ShutdownController;
use resolved_pathbuf::ResolvedPathBuf;
use tokio::pin;
use tracing::warn;

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

    panic_report::add_context("config", config.into_inner());

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

    let mut interval = tokio::time::interval(Duration::from_secs(30));
    let checkpoint_hash = [
        216, 45, 9, 251, 90, 41, 185, 209, 19, 80, 237, 170, 217, 58, 20, 97, 88, 104, 72, 164, 46,
        190, 215, 163, 74, 108, 68, 60, 179, 56, 213, 80,
    ];
    println!("###### 1");

    loop {
        tokio::select! {
            _ = &mut shutdown_future => break,
            _ = interval.tick() => {
                println!("###### 2");
                // get the checkpoint from the blockstore
                let checkpoint = node
                .container
                .get::<<C as Collection>::BlockStoreInterface>(tag!(C::BlockStoreInterface))
                .read_all_to_vec(&checkpoint_hash).await.expect("Failed to read checkpoint from blockstore");

                // wait for node to start
                tokio::time::sleep(Duration::from_secs(30)).await;

                // shutdown the node
                if let Some(cb) = &custom_start_shutdown {
                    ((cb)(&node, false)).await;
                } else {
                    node.shutdown().await;
                }
                println!("###### 3");

                std::mem::drop(node);
                println!("###### 4");

                // Sleep for a bit but provide some feedback, some of our proccesses take a few milliseconds to drop from memory
                warn!("Preparing to load checkpoint, Restarting services");
                //tokio::time::sleep(Duration::from_secs(3)).await;

                // start local env in checkpoint mode to seed database with the new checkpoint
                C::ApplicationInterface::load_from_checkpoint(
                    &app_config, checkpoint, checkpoint_hash).await?;

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

                increment_counter!(
                    "checkpoint_loaded",
                    Some("Counter for number of times the node restarted from a new checkpoint")
                );
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
