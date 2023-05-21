use affair::Socket;
use async_trait::async_trait;

use crate::{
    common::WithStartAndShutdown,
    config::ConfigConsumer,
    identity::PeerId,
    transaction::{Block, QueryRequest, QueryResponse},
};

/// The response generated from executing an entire batch of transactions (aka a block).
#[derive(Debug, PartialEq, PartialOrd, Hash, Eq)]
pub struct BlockExecutionResponse {
    /// This *flag* is only set to `true` if performing a transaction in the block
    /// has determined that we should move the epoch forward.
    change_epoch: bool,
}

/// The port that is handled by the application layer and fed by consensus (or other
/// synchronization systems in place) which executes and persists transactions that
/// are put into it.
///
/// # Safety
///
/// This port should be used with as much caution as possible, for all intend and purposes
/// this port should be sealed and preferably not accessible out side of the scope in which
/// it is created.
pub type ExecutionEnginePort = Socket<Block, BlockExecutionResponse>;

/// The port that is handled by the application layer and fed by consensus (or other
/// synchronization systems in place) which executes and persists transactions that
/// are put into it.
///
/// # Safety
///
/// This port should be used with as much caution as possible, for all intend and purposes
/// this port should be sealed and preferably not accessible out side of the scope in which
/// it is created.
pub type QueryPort = Socket<QueryRequest, QueryResponse>;

#[async_trait]
pub trait ApplicationInterface:
    WithStartAndShutdown + ConfigConsumer + Sized + Send + Sync
{
    /// The type for the balance getter.
    type BalanceGetter: SyncBalanceQueryRunner;

    /// Create a new instance of the application layer using the provided configuration.
    async fn init(config: Self::Config) -> anyhow::Result<Self>;

    /// Returns a port that should be used to submit transactions to be executed
    /// by the application layer.
    ///
    /// # Safety
    ///
    /// See the safety document for the [`ExecutionEnginePort`].
    fn transaction_executor(&self) -> ExecutionEnginePort;

    /// Returns a port that can be used to execute queries on the application layer. This
    /// port can be passed to the *RPC* as an example.
    fn query_port(&self) -> QueryPort;

    /// Returns an instance of the balance getter. The caller must be able to clone a balance
    /// getter and send it safely across many threads.
    fn get_balance_getter(&self) -> Self::BalanceGetter;
}

pub trait SyncBalanceQueryRunner: Clone + Send + Sync {
    /// Returns the latest balance associated with the given peer.
    fn get_balance(peer: &PeerId) -> u128;
}
