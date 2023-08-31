use lightning_handshake::handshake::Handshake;
use lightning_interfaces::infu_collection::Collection;
use lightning_interfaces::partial;
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

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    Cli::<ExampleBinding>::parse_and_exec().await
}
