use std::sync::Arc;

use affair::Socket;
use async_trait::async_trait;
use infusion::infu;
use lightning_schema::LightningMessage;
use tokio::sync::Notify;

use crate::{
    application::ExecutionEngineSocket, common::WithStartAndShutdown, config::ConfigConsumer,
    infu_collection::Collection, signer::SignerInterface, types::UpdateRequest, PubSub,
    SyncQueryRunnerInterface,
};

/// A socket that gives services and other sub-systems the required functionality to
/// submit messages/transactions to the consensus.
///
/// # Safety
///
/// This socket is safe to freely pass around, sending transactions through this socket
/// does not guarantee their execution on the application layer. You can think about
/// this as if the current node was only an external client to the network.
pub type MempoolSocket = Socket<UpdateRequest, ()>;

#[async_trait]
pub trait ConsensusInterface: WithStartAndShutdown + ConfigConsumer + Sized + Send + Sync {
    infu!(ConsensusInterface @ Input);

    // -- DYNAMIC TYPES

    type QueryRunner: SyncQueryRunnerInterface;

    type PubSub: PubSub<Self::Certificate>;

    // -- BOUNDED TYPES

    type Certificate: LightningMessage + Clone;

    /// Create a new consensus service with the provided config and executor.
    fn init<S: SignerInterface>(
        config: Self::Config,
        signer: &S,
        executor: ExecutionEngineSocket,
        query_runner: Self::QueryRunner,
        pubsub: Self::PubSub,
    ) -> anyhow::Result<Self>;

    /// Returns a socket that can be used to submit transactions to the consensus,
    /// this can be used by any other systems that are interested in posting some
    /// transaction to the consensus.
    fn mempool(&self) -> MempoolSocket;

    /// Returns a tokio Notifier that notifies everytime a new block is finished being processed
    fn new_block_notifier(&self) -> Arc<Notify>;
}
