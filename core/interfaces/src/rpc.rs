use infusion::c;

use crate::common::WithStartAndShutdown;
use crate::config::ConfigConsumer;
use crate::consensus::MempoolSocket;
use crate::infu_collection::Collection;
use crate::{
    ApplicationInterface,
    ArchiveInterface,
    ArchiveSocket,
    BlockStoreInterface,
    ConfigProviderInterface,
    ConsensusInterface,
    FetcherInterface,
    SignerInterface,
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
        archive: ::ArchiveInterface,
        signer: ::SignerInterface,
    ) {
        Self::init(
            config.get::<Self>(),
            consensus.mempool(),
            app.sync_query(),
            blockstore.clone(),
            fetcher,
            archive.archive_socket(),
            signer,
        )
    }

    /// Initialize the RPC-server, with the given parameters.
    fn init(
        config: Self::Config,
        mempool: MempoolSocket,
        query_runner: c!(C::ApplicationInterface::SyncExecutor),
        blockstore: C::BlockStoreInterface,
        fetcher: &C::FetcherInterface,
        archive_socket: Option<ArchiveSocket>,
        signer: &C::SignerInterface,
    ) -> anyhow::Result<Self>;
}
