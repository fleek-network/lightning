use affair::Socket;
use async_trait::async_trait;
use fleek_crypto::{ClientPublicKey, NodePublicKey};

use crate::{
    common::WithStartAndShutdown,
    config::ConfigConsumer,
    types::{Block, NodeInfo, QueryRequest, QueryResponse, TransactionResponse},
};

/// The response generated from executing an entire batch of transactions (aka a block).
#[derive(Debug, PartialEq, PartialOrd, Hash, Eq)]
pub struct BlockExecutionResponse {
    /// The new block hash
    pub block_hash: [u8; 32],
    /// This *flag* is only set to `true` if performing a transaction in the block
    /// has determined that we should move the epoch forward.
    pub change_epoch: bool,
    /// The changes to the node registry.
    pub node_registry_delta: Vec<(NodePublicKey, NodeRegistryChange)>,
    /// Reciepts of all executed transactions
    pub txn_receipts: Vec<TransactionResponse>,
}

#[derive(Debug, PartialEq, PartialOrd, Hash, Eq)]
pub enum NodeRegistryChange {
    New,
    Removed,
}

/// The socket that is handled by the application layer and fed by consensus (or other
/// synchronization systems in place) which executes and persists transactions that
/// are put into it.
///
/// # Safety
///
/// This socket should be used with as much caution as possible, for all intend and purposes
/// this socket should be sealed and preferably not accessible out side of the scope in which
/// it is created.
pub type ExecutionEngineSocket = Socket<Block, BlockExecutionResponse>;

/// The socket that is handled by the application layer and fed by consensus (or other
/// synchronization systems in place) which executes and persists transactions that
/// are put into it.
///
/// # Safety
///
/// This socket should be used with as much caution as possible, for all intend and purposes
/// this socket should be sealed and preferably not accessible out side of the scope in which
/// it is created.
pub type QuerySocket = Socket<QueryRequest, QueryResponse>;

#[async_trait]
pub trait ApplicationInterface:
    WithStartAndShutdown + ConfigConsumer + Sized + Send + Sync
{
    /// The type for the sync query executor.
    type SyncExecutor: SyncQueryRunnerInterface;

    /// Create a new instance of the application layer using the provided configuration.
    async fn init(config: Self::Config) -> anyhow::Result<Self>;

    /// Returns a socket that should be used to submit transactions to be executed
    /// by the application layer.
    ///
    /// # Safety
    ///
    /// See the safety document for the [`ExecutionEngineSocket`].
    fn transaction_executor(&self) -> ExecutionEngineSocket;

    /// Returns a socket that can be used to execute queries on the application layer. This
    /// socket can be passed to the *RPC* as an example.
    fn query_socket(&self) -> QuerySocket;

    /// Returns the instance of a sync query runner which can be used to run queries without
    /// blocking or awaiting. A naive (& blocking) implementation can achieve this by simply
    /// putting the entire application state in an `Arc<RwLock<T>>`, but that is not optimal
    /// and is the reason why we have `Atomo` to allow us to have the same kind of behavior
    /// without slowing down the system.
    fn sync_query(&self) -> Self::SyncExecutor;
}

pub trait SyncQueryRunnerInterface: Clone + Send + Sync {
    /// Returns the latest balance associated with the given peer.
    fn get_balance(&self, client: &ClientPublicKey) -> u128;

    /// Returns the global reputation of a node.
    fn get_reputation(&self, node: &NodePublicKey) -> u128;

    /// Returns the relative score between two nodes, this score should measure how much two
    /// nodes `n1` and `n2` trust each other. Of course in real world a direct measurement
    /// between any two node might not exits, but there does exits a path from `n1` to `n2`
    /// which may cross any `node_i`, a page rank like algorithm is needed here to measure
    /// a relative score between two nodes.
    ///
    /// Existence of this data can allow future optimizations of the network topology.
    fn get_relative_score(&self, n1: &NodePublicKey, n2: &NodePublicKey) -> u128;

    /// Returns information about a single node.
    fn get_node_info(&self, id: &NodePublicKey) -> Option<NodeInfo>;

    /// Returns a full copy of the entire node-registry, but only contains the nodes that
    /// are still a valid node and have enough stake.
    fn get_node_registry(&self) -> Vec<NodeInfo>;

    /// Returns true if the node is a valid node in the network, with enough stake.
    fn is_valid_node(&self, id: &NodePublicKey) -> bool;

    /// Returns the amount that is required to be a valid node in the network.
    fn get_staking_amount(&self) -> u128;

    /// Returns the randomness that was used to start the current epoch.
    fn get_epoch_randomness_seed(&self) -> &[u8; 32];

    /// Returns the committee members of the current epoch.
    fn get_committee_members(&self) -> Vec<NodePublicKey>;
}

#[derive(Clone, Debug)]
pub enum ExecutionError {
    InvalidSignature,
    InvalidNonce,
    InvalidProof,
    NotNodeOwner,
    NotCommitteeMember,
    NodeDoesNotExist,
    AlreadySignaled,
    NonExistingService,
}
