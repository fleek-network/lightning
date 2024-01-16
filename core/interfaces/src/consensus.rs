use std::sync::Arc;

use affair::Socket;
use infusion::c;
use lightning_schema::LightningMessage;
use tokio::sync::Notify;

use crate::application::ExecutionEngineSocket;
use crate::common::WithStartAndShutdown;
use crate::config::ConfigConsumer;
use crate::infu_collection::Collection;
use crate::signer::SignerInterface;
use crate::types::TransactionRequest;
use crate::{
    ApplicationInterface,
    ArchiveInterface,
    BroadcastInterface,
    ConfigProviderInterface,
    IndexSocket,
    NotifierInterface,
};

/// A socket that gives services and other sub-systems the required functionality to
/// submit messages/transactions to the consensus.
///
/// # Safety
///
/// This socket is safe to freely pass around, sending transactions through this socket
/// does not guarantee their execution on the application layer. You can think about
/// this as if the current node was only an external client to the network.
pub type MempoolSocket = Socket<TransactionRequest, ()>;

#[infusion::service]
pub trait ConsensusInterface<C: Collection>:
    WithStartAndShutdown + ConfigConsumer + Sized + Send + Sync
{
    fn _init(
        config: ::ConfigProviderInterface,
        signer: ::SignerInterface,
        app: ::ApplicationInterface,
        broadcast: ::BroadcastInterface,
        archive: ::ArchiveInterface,
        notifier: ::NotifierInterface,
    ) {
        let executor = app.transaction_executor();

        let sqr = app.sync_query();
        let pubsub = broadcast.get_pubsub(crate::types::Topic::Consensus);
        Self::init(
            config.get::<Self>(),
            signer,
            executor,
            sqr,
            pubsub,
            archive.index_socket(),
            notifier,
        )
    }

    type Certificate: LightningMessage + Clone;

    /// Create a new consensus service with the provided config and executor.
    fn init<S: SignerInterface<C>>(
        config: Self::Config,
        signer: &S,
        executor: ExecutionEngineSocket,
        query_runner: c!(C::ApplicationInterface::SyncExecutor),
        pubsub: c!(C::BroadcastInterface::PubSub<Self::Certificate>),
        indexer_socket: Option<IndexSocket>,
        notifier: &c!(C::NotifierInterface),
    ) -> anyhow::Result<Self>;

    /// Returns a socket that can be used to submit transactions to the consensus,
    /// this can be used by any other systems that are interested in posting some
    /// transaction to the consensus.
    #[blank = Socket::raw_bounded(64).0]
    fn mempool(&self) -> MempoolSocket;

    /// Returns a tokio Notifier that notifies every time a new block is finished being processed
    #[blank = Default::default()]
    fn new_block_notifier(&self) -> Arc<Notify>;
}
