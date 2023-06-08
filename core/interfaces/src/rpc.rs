use async_trait::async_trait;
use fleek_crypto::{AccountOwnerPublicKey, NodePublicKey};

use crate::{
    common::WithStartAndShutdown, config::ConfigConsumer, consensus::MempoolSocket,
    types::TransactionResponse, SyncQueryRunnerInterface,
};

/// The interface for the *RPC* server. Which is supposed to be opening a public
/// port (possibly an HTTP server) and accepts queries or updates from the user.
#[async_trait]
pub trait RpcInterface: Sized + ConfigConsumer + WithStartAndShutdown {
    /// Initialize the *RPC* server, with the given parameters.
    async fn init<Q: SyncQueryRunnerInterface>(
        config: Self::Config,
        mempool: MempoolSocket,
        query_runner: Q,
    ) -> anyhow::Result<Self>;
}
#[async_trait]
pub trait RpcMethods: Sync + Send + 'static {
    /// ping method for rpc server, for clients to check if server is online
    async fn ping(&self) -> anyhow::Result<String>;

    /// this method would fetch the account balance of a particular address or account.
    async fn get_balance(&self, public_key: AccountOwnerPublicKey) -> TransactionResponse;

    /// this method would fetch the bandwidth balance of a particular address or account.
    async fn get_bandwidth(&self, public_key: AccountOwnerPublicKey) -> TransactionResponse;

    /// this method would fetch the locked token balance of a particular node.
    async fn get_locked(&self, node_key: NodePublicKey) -> TransactionResponse;

    /// this method would fetch the staked token balance of a particular node.
    async fn get_staked(&self, node_key: NodePublicKey) -> TransactionResponse;

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
