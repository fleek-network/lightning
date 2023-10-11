use affair::Socket;
use infusion::c;
use lightning_types::{ArchiveRequest, ArchiveResponse, IndexRequest};

use crate::common::WithStartAndShutdown;
use crate::config::ConfigConsumer;
use crate::infu_collection::Collection;
use crate::{ApplicationInterface, ConfigProviderInterface};

pub type ArchiveSocket = Socket<ArchiveRequest, ArchiveResponse>;
pub type IndexSocket = Socket<IndexRequest, ()>;

#[infusion::service]
pub trait ArchiveInterface<C: Collection>:
    WithStartAndShutdown + ConfigConsumer + Sized + Send + Sync
{
    fn _init(config: ::ConfigProviderInterface, app: ::ApplicationInterface) {
        let sqr = app.sync_query();
        Self::init(config.get::<Self>(), sqr)
    }

    /// Create a new archive service, if this node is not configed to be an archive node we shouldnt
    /// do much here
    fn init(
        config: Self::Config,
        query_runner: c!(C::ApplicationInterface::SyncExecutor),
    ) -> anyhow::Result<Self>;

    /// Returns a socket that can be used to query about data that has been archived
    /// Will return none if the node is not currently configured to be an archive node
    fn archive_socket(&self) -> Option<ArchiveSocket>;

    /// Returns a socket that can be given to consensus to submit blocks and transactions it has
    /// executed to be archived
    fn index_socket(&self) -> Option<IndexSocket>;
}
