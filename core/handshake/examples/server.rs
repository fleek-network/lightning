// cargo run --example server -- --config ./examples/config.toml -vvvv run
// run static file server separately, e.g
// python3 -m http.server

use lightning_blockstore::blockstore::Blockstore;
use lightning_cli::cli::Cli;
use lightning_handshake::handshake::Handshake;
use lightning_interfaces::infu_collection::Collection;
use lightning_interfaces::{
    forward,
    partial,
    HandshakeInterface,
    ServiceExecutorInterface,
    WithStartAndShutdown,
};
use lightning_node::config::TomlConfigProvider;
use lightning_service_executor::shim::ServiceExecutor;
use lightning_signer::Signer;
use mock::syncronizer::MockSyncronizer;

partial!(ExampleBinding {
    ConfigProviderInterface = TomlConfigProvider<Self>;
    BlockStoreInterface = Blockstore<Self>;
    SignerInterface = Signer<Self>;
    SyncronizerInterface = MockSyncronizer<Self>;
    HandshakeInterface = Handshake<Self>;
    ServiceExecutorInterface = ServiceExecutor<Self>;
});

forward!(async fn start_or_shutdown_node(this, start: bool) on [
    HandshakeInterface,
    ServiceExecutorInterface,
] {
    if start {
        log::info!("starting {}", get_name(&this));
        this.start().await;
    } else {
        log::info!("shutting down {}", get_name(&this));
        this.shutdown().await;
    }
});

fn get_name<T>(_: &T) -> &str {
    std::any::type_name::<T>()
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    cli.exec_with_custom_start_shutdown::<ExampleBinding>(Box::new(|node, start| {
        Box::pin(async move {
            start_or_shutdown_node::<ExampleBinding>(&node.container, start).await;
        })
    }))
    .await
}
