use async_trait::async_trait;

use crate::{
    application::QueryPort, common::WithStartAndShutdown, config::ConfigConsumer,
    consensus::MempoolPort, identity::SignatureVerifierInterface,
};

/// The interface for the *RPC* server. Which is supposed to be opening a public
/// port (possibly an HTTP server) and accepts queries or updates from the user.
#[async_trait]
pub trait RpcInterface<SignVerifier: SignatureVerifierInterface>:
    Sized + ConfigConsumer + WithStartAndShutdown
{
    /// Initialize the *RPC* server, with the given parameters.
    async fn init(
        config: Self::Config,
        mempool: MempoolPort,
        query_port: QueryPort,
    ) -> anyhow::Result<Self>;
}
