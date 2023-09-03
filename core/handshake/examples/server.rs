// cargo run --example server -- --config ./examples/config.toml -vvvv run
// run static file server separately, e.g
// python3 -m http.server

use lightning_handshake::handshake::Handshake;
use lightning_interfaces::infu_collection::Collection;
use lightning_interfaces::{
    forward,
    partial,
    HandshakeInterface,
    ServiceExecutorInterface,
    WithStartAndShutdown,
};
use lightning_node::cli::Cli;
use lightning_node::config::TomlConfigProvider;
use lightning_service_executor::shim::ServiceExecutor;
use lightning_signer::Signer;

partial!(ExampleBinding {
    ConfigProviderInterface = TomlConfigProvider<Self>;
    SignerInterface = Signer<Self>;
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
    Cli::<ExampleBinding>::parse()
        .with_custom_start_shutdown(Box::new(|node, start| {
            Box::pin(async move {
                start_or_shutdown_node::<ExampleBinding>(&node.container, start).await;
            })
        }))
        .exec()
        .await
}
