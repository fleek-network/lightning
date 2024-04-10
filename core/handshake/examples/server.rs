// cargo run --example server -- --config ./examples/config.toml -vvvv run
// run static file server separately, e.g
// python3 -m http.server

use lightning_blockstore::blockstore::Blockstore;
use lightning_cli::cli::Cli;
use lightning_handshake::handshake::Handshake;
use lightning_interfaces::Collection;
use lightning_interfaces::partial;
use lightning_node::config::TomlConfigProvider;
use lightning_rpc::Rpc;
use lightning_service_executor::shim::ServiceExecutor;
use lightning_signer::Signer;

partial!(ExampleBinding {
    ConfigProviderInterface = TomlConfigProvider<Self>;
    BlockstoreInterface = Blockstore<Self>;
    SignerInterface = Signer<Self>;
    HandshakeInterface = Handshake<Self>;
    ServiceExecutorInterface = ServiceExecutor<Self>;
    RpcInterface = Rpc<Self>;
});

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    cli.exec().await
}
