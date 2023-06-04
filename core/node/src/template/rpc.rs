use async_trait::async_trait;
use draco_interfaces::{
    common::WithStartAndShutdown, config::ConfigConsumer, MempoolSocket, QuerySocket, RpcInterface,
};

use super::config::Config;

#[derive(Clone)]
pub struct Rpc {}

#[async_trait]
impl WithStartAndShutdown for Rpc {
    /// Returns true if this system is running or not.
    fn is_running(&self) -> bool {
        todo!()
    }

    /// Start the system, should not do anything if the system is already
    /// started.
    async fn start(&self) {
        todo!()
    }

    /// Send the shutdown signal to the system.
    async fn shutdown(&self) {
        todo!()
    }
}

#[async_trait]
impl RpcInterface for Rpc {
    /// Initialize the *RPC* server, with the given parameters.
    async fn init(
        _config: Self::Config,
        _mempool: MempoolSocket,
        _query_socket: QuerySocket,
    ) -> anyhow::Result<Self> {
        todo!()
    }
}

impl ConfigConsumer for Rpc {
    const KEY: &'static str = "rpc";

    type Config = Config;
}
