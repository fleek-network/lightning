use std::collections::HashMap;
use std::time::Duration;

use affair::Socket;
use anyhow::Result;
use fleek_crypto::{ClientPublicKey, EthAddress, NodePublicKey};
use lightning_types::{
    AccountInfo,
    Blake3Hash,
    Committee,
    NodeIndex,
    NodeInfoWithIndex,
    TransactionRequest,
    TxHash,
    Value,
};
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
    /// Query Metadata Table
    fn get_metadata(&self, key: &lightning_types::Metadata) -> Option<Value>;

    /// Query Account Table
    fn get_account_info<F: Clone>(
        &self,
        address: &EthAddress,
        selector: impl FnOnce(AccountInfo) -> F,
    ) -> Option<F>;

    /// Query Client Table
    fn get_client_info(self, pub_key: &ClientPublicKey) -> Option<EthAddress>;

    /// Query Node Table
    /// Returns information about a single node.
    fn get_node_info<F: Clone>(
        &self,
        node: &NodeIndex,
        selector: impl FnOnce(NodeInfo) -> F,
    ) -> Option<F>;

    /// Query Pub Key to Node Index Table
    fn pubkey_to_index(&self, pub_key: &NodePublicKey) -> Option<NodeIndex>;

    /// Query Committe Table
    fn get_committe_info<F: Clone>(
        &self,
        epoch: &Epoch,
        selector: impl FnOnce(Committee) -> F,
    ) -> Option<F>;

    /// Query Services Table
    fn get_service_info(&self, id: &ServiceId) -> Option<Service>;

    /// Query Params Table
    fn get_protocol_param(&self, param: &ProtocolParams) -> Option<u128>;

    /// Query Current Epoch Served Table
    fn get_current_epoch_served(&self, node: &NodeIndex) -> Option<NodeServed>;

    /// Query Reputation Measurements
    fn get_reputation_measurements(
        &self,
        node: &NodeIndex,
    ) -> Option<Vec<ReportedReputationMeasurements>>;

    /// Query Latencies Table
    fn get_latencies(&self, node_1: &NodeIndex, node_2: &NodeIndex) -> Option<Duration>;

    /// Query Reputation Scores Table
    fn get_reputation_score(&self, node: &NodeIndex) -> Option<u8>;

    /// Query Total Served Table
    fn get_total_served(&self, epoch: &Epoch) -> Option<TotalServed>;

    /// Returns the chain id
    fn get_chain_id(&self) -> u32;

    /// Autometrics
    /// Returns the committee members of the current epoch
    fn get_committee_members(&self) -> Vec<NodePublicKey>;

    /// Returns the committee members of the current epoch by NodeIndex
    fn get_committee_members_by_index(&self) -> Vec<NodeIndex>;

    /// Get Current Epoch
    fn get_current_epoch(&self) -> Epoch;

    /// Get Current Epoch Info
    fn get_epoch_info(&self) -> EpochInfo;

    /// Return all latencies measurements for the current epoch.
    fn get_current_latencies(&self) -> HashMap<(NodePublicKey, NodePublicKey), Duration>;

    /// Returns the node info of the genesis committee members
    fn get_genesis_committee(&self) -> Vec<(NodeIndex, NodeInfo)>;

    /// Returns last executed block hash. [0;32] is genesis
    fn get_last_block(&self) -> [u8; 32];

    // Returns a full copy of the entire node-registry,
    /// Paging Params - filtering nodes that are still a valid node and have enough stake; Takes
    /// from starting index and specified amount.
    fn get_node_registry(&self, paging: Option<PagingParams>) -> Vec<NodeInfoWithIndex>;

    /// Checks if an transaction digest has been executed this epoch.
    fn has_executed_digest(&self, digest: TxHash) -> bool;

    /// Get Node Public Key based on Node Index
    fn index_to_pubkey(&self, node_index: &NodeIndex) -> Option<NodePublicKey>;

    /// Returns true if the node is a valid node in the network, with enough stake.
    fn is_valid_node(&self, id: &NodePublicKey) -> bool;

    /// Simulate Transaction
    fn simulate_txn(&self, txn: TransactionRequest) -> TransactionResponse;

    /// Returns the uptime for a node from the past epoch.
    fn get_node_uptime(&self, node_index: &NodeIndex) -> Option<u8>;

    /// Returns nodes that are providing the content addressed by the cid.
    fn cid_to_providers(&self, cid: &Blake3Hash) -> Vec<NodeIndex>;

    /// Returns the node's content registry.
    fn content_registry(&self, node_index: &NodeIndex) -> Vec<Blake3Hash>;
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

#[derive(Deserialize, Serialize, schemars::JsonSchema)]
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
