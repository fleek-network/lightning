use std::sync::Arc;

use affair::Socket;
use infusion::c;
use lightning_schema::LightningMessage;
use tokio::sync::Notify;

use crate::{
    application::ExecutionEngineSocket, common::WithStartAndShutdown, config::ConfigConsumer,
    infu_collection::Collection, signer::SignerInterface, types::UpdateRequest,
    ApplicationInterface, BroadcastInterface, ConfigProviderInterface,
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

#[infusion::service]
pub trait ConsensusInterface<C: Collection>:
    WithStartAndShutdown + ConfigConsumer + Sized + Send + Sync
{
    fn _init(
        config: ::ConfigProviderInterface,
        signer: ::SignerInterface,
        app: ::ApplicationInterface,
        broadcast: ::BroadcastInterface,
    ) {
        let executor = app.transaction_executor();
        let sqr = app.sync_query();
        let pubsub = broadcast.get_pubsub(crate::types::Topic::Consensus);
        Self::init(config.get::<Self>(), signer, executor, sqr, pubsub)
    }

    type Certificate: LightningMessage + Clone;

    /// Create a new consensus service with the provided config and executor.
    fn init<S: SignerInterface<C>>(
        config: Self::Config,
        signer: &S,
        executor: ExecutionEngineSocket,
        query_runner: c!(C::ApplicationInterface::SyncExecutor),
        pubsub: c!(C::BroadcastInterface::PubSub<Self::Certificate>),
    ) -> anyhow::Result<Self>;

    /// Returns a socket that can be used to submit transactions to the consensus,
    /// this can be used by any other systems that are interested in posting some
    /// transaction to the consensus.
    fn mempool(&self) -> MempoolSocket;

    /// Returns a tokio Notifier that notifies everytime a new block is finished being processed
    fn new_block_notifier(&self) -> Arc<Notify>;
}
