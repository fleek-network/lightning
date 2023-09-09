use infusion::c;

use crate::common::WithStartAndShutdown;
use crate::config::ConfigConsumer;
use crate::consensus::MempoolSocket;
use crate::infu_collection::Collection;
use crate::{ApplicationInterface, ConfigProviderInterface, ConsensusInterface, FetcherInterface};

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
        fetcher: ::FetcherInterface,
    ) {
        Self::init(
            config.get::<Self>(),
            consensus.mempool(),
            app.sync_query(),
            fetcher,
        )
    }

    /// Initialize the RPC-server, with the given parameters.
    fn init(
        config: Self::Config,
        mempool: MempoolSocket,
        query_runner: c!(C::ApplicationInterface::SyncExecutor),
        fetcher: &C::FetcherInterface,
    ) -> anyhow::Result<Self>;
}
