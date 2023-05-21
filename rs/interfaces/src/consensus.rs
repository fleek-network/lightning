use affair::Socket;
use async_trait::async_trait;

use crate::{
    application::ExecutionEnginePort, common::WithStartAndShutdown, config::ConfigConsumer,
    transaction::UpdateMethod,
};

/// A port that gives services and other sub-systems the required functionality to
/// submit messages/transactions to the consensus.
///
/// # Safety
///
/// This port is safe to freely pass around, sending transactions through this port
/// does not guarantee their execution on the application layer. You can think about
/// this as if the current node was only an external client to the network.
pub type MempoolPort = Socket<UpdateMethod, ()>;

#[async_trait]
pub trait ConsensusInterface: WithStartAndShutdown + ConfigConsumer + Sized + Send + Sync {
    /// Create a new consensus service with the provided config and executor.
    async fn init(config: Self::Config, executor: ExecutionEnginePort) -> anyhow::Result<Self>;

    /// Returns a port that can be used to submit transactions to the consensus,
    /// this can be used by any other systems that are interested in posting some
    /// transaction to the consensus.
    fn mempool(&self) -> MempoolPort;
}
