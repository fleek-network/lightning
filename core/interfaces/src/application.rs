use affair::Socket;
use async_trait::async_trait;
use big_decimal::BigDecimal;
use fleek_crypto::{AccountOwnerPublicKey, ClientPublicKey, NodePublicKey};

use crate::{
    common::WithStartAndShutdown,
    config::ConfigConsumer,
    types::{
        Block, CommodityServed, Epoch, EpochInfo, NodeInfo, ProtocolParams,
        ReportedReputationMeasurements, TotalServed, TransactionResponse,
    },
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

    /// Returns the instance of a sync query runner which can be used to run queries without
    /// blocking or awaiting. A naive (& blocking) implementation can achieve this by simply
    /// putting the entire application state in an `Arc<RwLock<T>>`, but that is not optimal
    /// and is the reason why we have `Atomo` to allow us to have the same kind of behavior
    /// without slowing down the system.
    fn sync_query(&self) -> Self::SyncExecutor;
}

pub trait SyncQueryRunnerInterface: Clone + Send + Sync {
    /// Returns the latest bandwidth balance associated with the given account public key.
    fn get_account_balance(&self, account: &AccountOwnerPublicKey) -> u128;

    /// Returns the latest bandwidth balance associated with the given client public key.
    fn get_client_balance(&self, client: &ClientPublicKey) -> u128;

    /// Returns the latest FLK balance of an account
    fn get_flk_balance(&self, account: &AccountOwnerPublicKey) -> BigDecimal<18>;

    /// Returns the latest stables balance of an account
    fn get_stables_balance(&self, account: &AccountOwnerPublicKey) -> BigDecimal<6>;

    /// Returns the amount of flk a node has staked
    fn get_staked(&self, node: &NodePublicKey) -> BigDecimal<18>;

    /// Returns the amount of locked tokens a node has
    fn get_locked(&self, node: &NodePublicKey) -> BigDecimal<18>;

    /// Returns the epoch number until which the stakes are locked
    fn get_stake_locked_until(&self, node: &NodePublicKey) -> Epoch;

    /// Returns the epoch a nodes tokens are unlocked at
    fn get_locked_time(&self, node: &NodePublicKey) -> Epoch;

    /// Returns the reported reputation measurements for a node.
    fn get_rep_measurements(&self, node: NodePublicKey) -> Vec<ReportedReputationMeasurements>;

    /// Returns the global reputation of a node.
    fn get_reputation(&self, node: &NodePublicKey) -> Option<u8>;

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

    /// Returns all the information on the current epoch that Narwhal needs to run
    fn get_epoch_info(&self) -> EpochInfo;

    /// Returns total served for all commodites from the state for a given epoch
    fn get_total_served(&self, epoch: Epoch) -> TotalServed;

    /// Return all commodity served for a give node for current epoch
    fn get_commodity_served(&self, node: &NodePublicKey) -> CommodityServed;

    /// Return the current total supply of FLK tokens
    fn get_total_supply(&self) -> BigDecimal<18>;

    /// Return the total supply at year start point used for inflation
    fn get_year_start_supply(&self) -> BigDecimal<18>;

    /// Returns the passed in protocol parameter
    fn get_protocol_params(&self, param: ProtocolParams) -> String;
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
