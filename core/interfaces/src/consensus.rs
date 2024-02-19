use affair::Socket;
use infusion::c;
use lightning_schema::LightningMessage;

use crate::application::ExecutionEngineSocket;
use crate::common::WithStartAndShutdown;
use crate::config::ConfigConsumer;
use crate::infu_collection::Collection;
use crate::signer::SignerInterface;
use crate::types::{Event, TransactionRequest};
use crate::{
    ApplicationInterface,
    ArchiveInterface,
    BroadcastInterface,
    ConfigProviderInterface,
    IndexSocket,
    KeystoreInterface,
    NotifierInterface,
    RpcInterface,
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
        keystore: ::KeystoreInterface,
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
            keystore.clone(),
            signer,
            executor,
            sqr,
            pubsub,
            archive.index_socket(),
            notifier,
        )
    }

    fn _post(&mut self, rpc: ::RpcInterface) {
        self.set_event_tx(rpc.event_tx());
    }

    type Certificate: LightningMessage + Clone;

    /// Create a new consensus service with the provided config and executor.
    #[allow(clippy::too_many_arguments)]
    fn init(
        config: Self::Config,
        keystore: C::KeystoreInterface,
        signer: &C::SignerInterface,
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

    fn set_event_tx(&mut self, tx: tokio::sync::mpsc::Sender<Vec<Event>>);
}
