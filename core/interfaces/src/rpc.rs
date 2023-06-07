use async_trait::async_trait;
use fleek_crypto::AccountOwnerPublicKey;

use crate::{
    application::QuerySocket, common::WithStartAndShutdown, config::ConfigConsumer,
    consensus::MempoolSocket, types::TransactionResponse,
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
#[async_trait]
pub trait RpcMethods: Sync + Send + 'static {
    /// ping method for rpc server, for clients to check if server is online
    async fn ping(&self) -> anyhow::Result<String>;

    /// this method would fetch the account balance of a particular address or account.
    async fn get_balance(&self, public_key: AccountOwnerPublicKey) -> TransactionResponse;

    /// This method would return information about a specific node.
    async fn get_node_info(&self);

    /// This method would return global information about the network
    async fn get_network_info(&self);

    /// This method would allow a client to submit a transaction to the network.
    async fn submit_transaction(&self);

    /// This method would allow a client to retrieve information about a specific block by its
    /// height or hash.
    async fn get_block(&self);

    /// This endpoint would allow a client to retrieve information about a specific transaction by
    /// its hash.
    async fn get_transaction(&self);
}
