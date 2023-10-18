use std::collections::HashMap;
use std::time::Duration;

use affair::Socket;
use anyhow::Result;
use fleek_crypto::{ClientPublicKey, EthAddress, NodePublicKey};
use hp_fixed::unsigned::HpUfixed;
use lightning_types::{AccountInfo, NodeIndex, TransactionRequest};
use serde::{Deserialize, Serialize};

use crate::common::WithStartAndShutdown;
use crate::config::ConfigConsumer;
use crate::infu_collection::Collection;
use crate::types::{
    Block,
    BlockExecutionResponse,
    Epoch,
    EpochInfo,
    NodeInfo,
    NodeServed,
    ProtocolParams,
    ReportedReputationMeasurements,
    Service,
    ServiceId,
    TotalServed,
    TransactionResponse,
};
use crate::{BlockStoreInterface, ConfigProviderInterface};

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

#[infusion::service]
pub trait ApplicationInterface<C: Collection>:
    WithStartAndShutdown + ConfigConsumer + Sized + Send + Sync
{
    fn _init(config: ::ConfigProviderInterface, blockstore: ::BlockStoreInterface) {
        let config = config.get::<Self>();
        Self::init(config, blockstore.clone())
    }

    /// The type for the sync query executor.
    type SyncExecutor: SyncQueryRunnerInterface;

    /// Create a new instance of the application layer using the provided configuration.
    fn init(config: Self::Config, blockstore: C::BlockStoreInterface) -> anyhow::Result<Self>;

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
    #[blank = Default::default()]
    fn sync_query(&self) -> Self::SyncExecutor;

    /// Will seed its underlying database with the checkpoint provided
    fn load_from_checkpoint(
        config: &Self::Config,
        checkpoint: Vec<u8>,
        checkpoint_hash: [u8; 32],
    ) -> Result<()>;
}

#[infusion::blank]
pub trait SyncQueryRunnerInterface: Clone + Send + Sync + 'static {
    /// Returns the latest bandwidth balance associated with the given account public key.
    fn get_account_balance(&self, account: &EthAddress) -> u128;

    /// Returns the latest bandwidth balance associated with the given client public key.
    fn get_client_balance(&self, client: &ClientPublicKey) -> u128;

    /// Returns the latest FLK balance of an account
    fn get_flk_balance(&self, account: &EthAddress) -> HpUfixed<18>;

    /// Returns the latest stables balance of an account
    fn get_stables_balance(&self, account: &EthAddress) -> HpUfixed<6>;

    /// Returns the amount of flk a node has staked
    fn get_staked(&self, node: &NodePublicKey) -> HpUfixed<18>;

    /// Returns the amount of locked tokens a node has
    fn get_locked(&self, node: &NodePublicKey) -> HpUfixed<18>;

    /// Returns the epoch number until which the stakes are locked
    fn get_stake_locked_until(&self, node: &NodePublicKey) -> Epoch;

    /// Returns the epoch a nodes tokens are unlocked at
    fn get_locked_time(&self, node: &NodePublicKey) -> Epoch;

    /// Returns the reported reputation measurements for a node.
    fn get_rep_measurements(&self, node: &NodeIndex) -> Vec<ReportedReputationMeasurements>;

    /// Returns the global reputation of a node.
    fn get_reputation(&self, node: &NodeIndex) -> Option<u8>;

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

    /// Returns information about an account.
    fn get_account_info(&self, id: &EthAddress) -> Option<AccountInfo>;

    /// Returns a full copy of the entire node-registry, but only contains the nodes that
    /// are still a valid node and have enough stake.
    fn get_node_registry(&self, paging: Option<PagingParams>) -> Vec<NodeInfo>;

    /// Returns true if the node is a valid node in the network, with enough stake.
    fn is_valid_node(&self, id: &NodePublicKey) -> bool;

    /// Returns the amount that is required to be a valid node in the network.
    fn get_staking_amount(&self) -> u128;

    /// Returns the randomness that was used to start the current epoch.
    fn get_epoch_randomness_seed(&self) -> &[u8; 32];

    /// Returns the committee members of the current epoch.
    fn get_committee_members(&self) -> Vec<NodePublicKey>;

    /// Returns the committee members of the current epoch by NodeIndex
    fn get_committee_members_by_index(&self) -> Vec<NodeIndex>;

    /// Returns just the current epoch
    fn get_epoch(&self) -> Epoch;

    /// Returns all the information on the current epoch that Narwhal needs to run
    fn get_epoch_info(&self) -> EpochInfo;

    /// Returns total served for all commodities from the state for a given epoch
    fn get_total_served(&self, epoch: Epoch) -> TotalServed;

    /// Return all commodity served for a give node for current epoch
    fn get_node_served(&self, node: &NodePublicKey) -> NodeServed;

    /// Return the current total supply of FLK tokens
    fn get_total_supply(&self) -> HpUfixed<18>;

    /// Return the total supply at year start point used for inflation
    fn get_year_start_supply(&self) -> HpUfixed<18>;

    /// Return the foundation address where protocol fund goes to
    fn get_protocol_fund_address(&self) -> EthAddress;

    /// Returns the passed in protocol parameter
    fn get_protocol_params(&self, param: ProtocolParams) -> u128;

    /// Validates the passed in transaction
    fn validate_txn(&self, txn: TransactionRequest) -> TransactionResponse;

    /// Return all latencies measurements for the current epoch.
    fn get_latencies(&self) -> HashMap<(NodePublicKey, NodePublicKey), Duration>;

    /// returns the service information for a given [`ServiceId`]
    fn get_service_info(&self, service_id: ServiceId) -> Service;

    fn pubkey_to_index(&self, node: NodePublicKey) -> Option<u32>;

    fn index_to_pubkey(&self, node_index: u32) -> Option<NodePublicKey>;

    /// Returns the hash of the last epoch.
    fn get_last_epoch_hash(&self) -> [u8; 32];

    /// takes NodeInfo and returns if they are a current committee member or not
    fn is_committee(&self, node_index: u32) -> bool;

    /// Returns the node info of the genesis committee members
    fn genesis_committee(&self) -> Vec<(NodeIndex, NodeInfo)>;

    /// Returns last executed block hash. [0;32] is genesis
    fn get_last_block(&self) -> [u8; 32];

    /// Returns an account's current nonce
    fn get_account_nonce(&self, public_key: &EthAddress) -> u64;

    /// Returns a node's current nonce
    fn get_node_nonce(&self, node_index: &NodeIndex) -> u64;

    /// Returns the chain id
    fn get_chain_id(&self) -> u32;

    /// Returns the current block number
    fn get_block_number(&self) -> u64;

    fn get_allow_mint(&self) -> bool;
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

#[derive(Deserialize, Serialize)]
pub struct PagingParams {
    // Since some nodes may be in state without
    // having staked the minimum and if at any point
    // they stake the minimum amount, this would
    // cause inconsistent results.
    // This flag allows you to query for all nodes
    // to keep returned results consistent.
    pub ignore_stake: bool,
    pub start: NodeIndex,
    pub limit: usize,
}
