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
    HandshakeInterface = Handshake<Self>;
    ServiceExecutorInterface = ServiceExecutor<Self>;
    SignerInterface = Signer<Self>;
});

forward!(async fn start_or_shutdown_node(this, start: bool) on [
    HandshakeInterface,
    ServiceExecutorInterface,
] {
    if start {
        this.start().await;
    } else {
        this.shutdown().await;
    }
});

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
