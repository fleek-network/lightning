use infusion::c;
use lightning_schema::LightningMessage;

use crate::application::ExecutionEngineSocket;
use crate::common::WithStartAndShutdown;
use crate::config::ConfigConsumer;
use crate::infu_collection::Collection;
use crate::signer::SignerInterface;
use crate::types::Event;
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

    fn set_event_tx(&mut self, tx: tokio::sync::mpsc::Sender<Vec<Event>>);
}
