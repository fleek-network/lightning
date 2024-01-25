use affair::Socket;
use ethers::types::BlockNumber;
use infusion::c;

use crate::common::WithStartAndShutdown;
use crate::config::ConfigConsumer;
use crate::infu_collection::Collection;
use crate::types::{
    Block,
    BlockExecutionResponse,
    BlockReceipt,
    TransactionReceipt,
    TransactionRequest,
};
use crate::{ApplicationInterface, BlockStoreInterface, ConfigProviderInterface};

pub type ArchiveSocket<C> = Socket<ArchiveRequest, anyhow::Result<ArchiveResponse<C>>>;
pub type IndexSocket = Socket<IndexRequest, anyhow::Result<()>>;

#[derive(Clone, Debug)]
pub enum ArchiveRequest {
    GetBlockByHash([u8; 32]),
    GetBlockByNumber(BlockNumber),
    GetTransactionReceipt([u8; 32]),
    GetTransaction([u8; 32]),
    GetHistoricalEpochState(u64),
}

pub enum ArchiveResponse<C: Collection> {
    HistoricalEpochState(c!(C::ApplicationInterface::SyncExecutor)),
    Block(BlockReceipt),
    TransactionReceipt(TransactionReceipt),
    Transaction(TransactionRequest),
    None,
}

impl<C: Collection> ArchiveResponse<C> {
    pub fn is_none(&self) -> bool {
        matches!(self, Self::None)
    }
}

#[derive(Clone, Debug)]
pub enum IndexRequest {
    Block(Block, BlockExecutionResponse),
    Epoch(u64, [u8; 32]),
}

#[infusion::service]
pub trait ArchiveInterface<C: Collection>:
    WithStartAndShutdown + ConfigConsumer + Sized + Send + Sync
{
    fn _init(
        config: ::ConfigProviderInterface,
        app: ::ApplicationInterface,
        blockstore: ::BlockStoreInterface,
    ) {
        let sqr = app.sync_query();
        Self::init(config.get::<Self>(), sqr, blockstore.clone())
    }

    /// Create a new archive service, if this node is not configed to be an archive node we shouldnt
    /// do much here
    fn init(
        config: Self::Config,
        query_runner: c!(C::ApplicationInterface::SyncExecutor),
        blockstore: c!(C::BlockStoreInterface),
    ) -> anyhow::Result<Self>;

    /// Returns a socket that can be used to query about data that has been archived
    /// Will return none if the node is not currently configured to be an archive node
    fn archive_socket(&self) -> Option<ArchiveSocket<C>>;

    /// Returns a socket that can be given to consensus to submit blocks and transactions it has
    /// executed to be archived
    fn index_socket(&self) -> Option<IndexSocket>;
}
