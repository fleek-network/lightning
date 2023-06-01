use async_trait::async_trait;
use draco_interfaces::{
    application::ExecutionEngineSocket,
    common::WithStartAndShutdown,
    config::ConfigConsumer,
    consensus::{ConsensusInterface, MempoolSocket},
    signer::SignerInterface,
};

use super::config::Config;

pub struct Consensus {}

#[async_trait]
impl WithStartAndShutdown for Consensus {
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

impl ConfigConsumer for Consensus {
    const KEY: &'static str = "consensus";

    type Config = Config;
}

#[async_trait]
impl ConsensusInterface for Consensus {
    /// Create a new consensus service with the provided config and executor.
    async fn init<S: SignerInterface>(
        config: Self::Config,
        signer: &S,
        executor: ExecutionEngineSocket,
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
