use affair::Socket;
use async_trait::async_trait;

use crate::{
    common::WithStartAndShutdown,
    config::ConfigConsumer,
    identity::{BlsPublicKey, PeerId},
    types::{Block, NodeInfo, QueryRequest, QueryResponse},
};

/// The response generated from executing an entire batch of transactions (aka a block).
#[derive(Debug, PartialEq, PartialOrd, Hash, Eq)]
pub struct BlockExecutionResponse {
    /// This *flag* is only set to `true` if performing a transaction in the block
    /// has determined that we should move the epoch forward.
    change_epoch: bool,
    /// The changes to the node registry.
    node_registry_delta: Vec<(BlsPublicKey, NodeRegistryChange)>,
}

#[derive(Debug, PartialEq, PartialOrd, Hash, Eq)]
pub enum NodeRegistryChange {
    NewNode,
    Slashed,
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
    /// The type for the sync query executor.
    type SyncExecutor: SyncQueryRunnerInterface;

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

    /// Returns the instance of a sync query runner which can be used to run queries without
    /// blocking or awaiting. A naive (& blocking) implementation can achieve this by simply
    /// putting the entire application state in an `Arc<RwLock<T>>`, but that is not optimal
    /// and is the reason why we have `Atomo` to allow us to have the same kind of behavior
    /// without slowing down the system.
    fn sync_query(&self) -> Self::SyncExecutor;
}

pub trait SyncQueryRunnerInterface: Clone + Send + Sync {
    /// Returns the latest balance associated with the given peer.
    fn get_balance(&self, peer: &PeerId) -> u128;

    /// Returns the global reputation of a node.
    fn get_reputation(&self, peer: &PeerId) -> u128;

    /// Returns the relative score between two nodes, this score should measure how much two
    /// nodes `n1` and `n2` trust each other. Of course in real world a direct measurement
    /// between any two node might not exits, but there does exits a path from `n1` to `n2`
    /// which may cross any `node_i`, a page rank like algorithm is needed here to measure
    /// a relative score between two nodes.
    ///
    /// Existence of this data can allow future optimizations of the network topology.
    fn get_relative_score(&self, n1: &PeerId, n2: &PeerId) -> u128;

    /// Returns information about a single node.
    fn get_node_info(&self, id: &PeerId) -> Option<NodeInfo>;

    /// Returns a full copy of the entire node-registry, but only contains the nodes that
    /// are still a valid node and have enough stake.
    fn get_node_registry(&self) -> Vec<NodeInfo>;

    /// Returns true if the node is a valid node in the network, with enough stake.
    fn is_valid_node(&self, id: &PeerId) -> bool;

    /// Returns the amount that is required to be a valid node in the network.
    fn get_staking_amount(&self) -> u128;

    /// Returns the randomness that was used to start the current epoch.
    fn get_epoch_randomness_seed(&self) -> &[u8; 32];

    /// Returns the committee members of the current epoch.
    fn get_committee_members(&self) -> Vec<BlsPublicKey>;
}
