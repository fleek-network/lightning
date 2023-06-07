use async_trait::async_trait;
use draco_interfaces::{
    application::ExecutionEngineSocket,
    common::WithStartAndShutdown,
    config::ConfigConsumer,
    consensus::{ConsensusInterface, MempoolSocket},
    signer::SignerInterface,
    SyncQueryRunnerInterface,
};

use crate::config::Config;

pub struct Consensus<Q: SyncQueryRunnerInterface, S: SignerInterface> {
    _query_runner: Q,
    _signer: S,
}

#[async_trait]
impl<Q: SyncQueryRunnerInterface, S: SignerInterface> WithStartAndShutdown for Consensus<Q, S> {
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

impl<Q: SyncQueryRunnerInterface, S: SignerInterface> ConfigConsumer for Consensus<Q, S> {
    const KEY: &'static str = "consensus";

    type Config = Config;
}

#[async_trait]
impl<R: SyncQueryRunnerInterface, I: SignerInterface> ConsensusInterface for Consensus<R, I> {
    /// Create a new consensus service with the provided config and executor.
    async fn init<S: SignerInterface, Q: SyncQueryRunnerInterface>(
        _config: Self::Config,
        _signer: &S,
        _executor: ExecutionEngineSocket,
        _query_runner: Q,
    ) -> anyhow::Result<Self> {
        todo!()
    }

    /// Returns a socket that can be used to submit transactions to the consensus,
    /// this can be used by any other systems that are interested in posting some
    /// transaction to the consensus.
    fn mempool(&self) -> MempoolSocket {
        todo!()
    }
}
