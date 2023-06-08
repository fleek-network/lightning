use async_trait::async_trait;
use draco_interfaces::{
    common::WithStartAndShutdown, config::ConfigConsumer, MempoolSocket, RpcInterface,
    SyncQueryRunnerInterface,
};

use super::config::Config;

#[derive(Clone)]
pub struct Rpc<Q: SyncQueryRunnerInterface> {
    _mempool_address: MempoolSocket,
    _query_runner: Q,
}

#[async_trait]
impl<Q: SyncQueryRunnerInterface> WithStartAndShutdown for Rpc<Q> {
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
impl<Q: SyncQueryRunnerInterface> RpcInterface<Q> for Rpc<Q> {
    /// Initialize the *RPC* server, with the given parameters.
    async fn init(
        _config: Self::Config,
        _mempool: MempoolSocket,
        _query_runner: Q,
    ) -> anyhow::Result<Self> {
        todo!()
    }

    fn query_runner(&self) -> Q {
        todo!()
    }
}

impl<Q: SyncQueryRunnerInterface> ConfigConsumer for Rpc<Q> {
    const KEY: &'static str = "rpc";

    type Config = Config;
}
