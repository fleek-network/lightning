use async_trait::async_trait;

use crate::{
    application::QuerySocket, common::WithStartAndShutdown, config::ConfigConsumer,
    consensus::MempoolSocket,
};

/// The interface for the *RPC* server. Which is supposed to be opening a public
/// port (possibly an HTTP server) and accepts queries or updates from the user.
#[async_trait]
pub trait RpcInterface: Sized + ConfigConsumer + WithStartAndShutdown {
    /// Initialize the *RPC* server, with the given parameters.
    async fn init(
        config: Self::Config,
        mempool: MempoolSocket,
        query_socket: QuerySocket,
    ) -> anyhow::Result<Self>;
}
