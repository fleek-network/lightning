use infusion::c;
use tokio::sync::mpsc;

use crate::common::WithStartAndShutdown;
use crate::config::ConfigConsumer;
use crate::consensus::MempoolSocket;
use crate::infu_collection::Collection;
use crate::types::Event;
use crate::{
    ApplicationInterface,
    ArchiveInterface,
    ArchiveSocket,
    BlockStoreInterface,
    ConfigProviderInterface,
    ConsensusInterface,
    FetcherInterface,
    KeystoreInterface,
};

/// The interface for the *RPC* server. Which is supposed to be opening a public
/// port (possibly an HTTP server) and accepts queries or updates from the user.
#[infusion::service]
pub trait RpcInterface<C: Collection>:
    Sized + Send + Sync + ConfigConsumer + WithStartAndShutdown
{
    fn _init(
        config: ::ConfigProviderInterface,
        consensus: ::ConsensusInterface,
        app: ::ApplicationInterface,
        blockstore: ::BlockStoreInterface,
        fetcher: ::FetcherInterface,
        keystore: ::KeystoreInterface,
        archive: ::ArchiveInterface,
    ) {
        Self::init(
            config.get::<Self>(),
            consensus.mempool(),
            app.sync_query(),
            blockstore.clone(),
            fetcher,
            keystore.clone(),
            archive.archive_socket(),
        )
    }

    /// Initialize the RPC-server, with the given parameters.
    fn init(
        config: Self::Config,
        mempool: MempoolSocket,
        query_runner: c!(C::ApplicationInterface::SyncExecutor),
        blockstore: C::BlockStoreInterface,
        fetcher: &C::FetcherInterface,
        keystore: C::KeystoreInterface,
        archive_socket: Option<ArchiveSocket<C>>,
    ) -> anyhow::Result<Self>;

    fn event_tx(&self) -> mpsc::Sender<Vec<Event>>;
}
