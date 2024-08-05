use std::collections::BTreeSet;
use std::path::Path;
use std::time::Duration;

use affair::Socket;
use anyhow::Result;
use atomo::{
    Atomo,
    AtomoBuilder,
    DefaultSerdeBackend,
    InMemoryStorage,
    KeyIterator,
    QueryPerm,
    SerdeBackend,
    StorageBackend,
    StorageBackendConstructor,
};
use fdi::BuildGraph;
use fleek_crypto::{ClientPublicKey, ConsensusPublicKey, EthAddress, NodePublicKey};
use hp_fixed::unsigned::HpUfixed;
use lightning_types::{
    AccountInfo,
    Blake3Hash,
    ChainId,
    Committee,
    CommodityTypes,
    Metadata,
    NodeIndex,
    ServiceRevenue,
    StateProofKey,
    StateProofValue,
    TransactionRequest,
    TxHash,
    Value,
};
use merklize::{MerklizeProvider, StateRootHash};
use serde::{Deserialize, Serialize};

use crate::collection::Collection;
use crate::types::{
    Block,
    BlockExecutionResponse,
    Epoch,
    NodeInfo,
    NodeServed,
    ProtocolParams,
    ReportedReputationMeasurements,
    Service,
    ServiceId,
    TotalServed,
    TransactionResponse,
};
use crate::ConfigConsumer;

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

#[interfaces_proc::blank]
pub trait ApplicationInterface<C: Collection>:
    BuildGraph + ConfigConsumer + Sized + Send + Sync
{
    /// The type for the sync query executor.
    type SyncExecutor: SyncQueryRunnerInterface;

    /// Returns a socket that should be used to submit transactions to be executed
    /// by the application layer.
    ///
    /// # Safety
    ///
    /// See the safety document for the [`ExecutionEngineSocket`].
    #[socket]
    fn transaction_executor(&self) -> ExecutionEngineSocket;

    /// Returns the instance of a sync query runner which can be used to run queries without
    /// blocking or awaiting. A naive (& blocking) implementation can achieve this by simply
    /// putting the entire application state in an `Arc<RwLock<T>>`, but that is not optimal
    /// and is the reason why we have `Atomo` to allow us to have the same kind of behavior
    /// without slowing down the system.
    fn sync_query(&self) -> Self::SyncExecutor;

    /// Will seed its underlying database with the checkpoint provided
    async fn load_from_checkpoint(
        config: &Self::Config,
        checkpoint: Vec<u8>,
        checkpoint_hash: [u8; 32],
    ) -> Result<()>;

    /// Used to get the chain id from the genesis file instead of state
    fn get_chain_id(config: &Self::Config) -> Result<ChainId>;

    /// Returns the committee from the geneis of the network
    fn get_genesis_committee(config: &Self::Config) -> Result<Vec<NodeInfo>>;
}

#[interfaces_proc::blank]
pub trait SyncQueryRunnerInterface: Clone + Send + Sync + 'static {
    #[blank(InMemoryStorage)]
    type Backend: StorageBackend + Send + Sync;

    fn new(atomo: Atomo<QueryPerm, Self::Backend>) -> Self;

    fn atomo_from_checkpoint(
        path: impl AsRef<Path>,
        hash: [u8; 32],
        checkpoint: &[u8],
    ) -> Result<Atomo<QueryPerm, Self::Backend>>;

    fn atomo_from_path(path: impl AsRef<Path>) -> Result<Atomo<QueryPerm, Self::Backend>>;

    fn register_tables<B: StorageBackendConstructor, S: SerdeBackend>(
        builder: AtomoBuilder<B, S>,
    ) -> AtomoBuilder<B, S> {
        builder
            .with_table::<Metadata, Value>("metadata")
            .with_table::<EthAddress, AccountInfo>("account")
            .with_table::<ClientPublicKey, EthAddress>("client_keys")
            .with_table::<NodeIndex, NodeInfo>("node")
            .with_table::<ConsensusPublicKey, NodeIndex>("consensus_key_to_index")
            .with_table::<NodePublicKey, NodeIndex>("pub_key_to_index")
            .with_table::<(NodeIndex, NodeIndex), Duration>("latencies")
            .with_table::<Epoch, Committee>("committee")
            .with_table::<ServiceId, Service>("service")
            .with_table::<ProtocolParams, u128>("parameter")
            .with_table::<NodeIndex, Vec<ReportedReputationMeasurements>>("rep_measurements")
            .with_table::<NodeIndex, u8>("rep_scores")
            .with_table::<NodeIndex, u8>("submitted_rep_measurements")
            .with_table::<NodeIndex, NodeServed>("current_epoch_served")
            .with_table::<NodeIndex, NodeServed>("last_epoch_served")
            .with_table::<Epoch, TotalServed>("total_served")
            .with_table::<CommodityTypes, HpUfixed<6>>("commodity_prices")
            .with_table::<ServiceId, ServiceRevenue>("service_revenue")
            .with_table::<TxHash, ()>("executed_digests")
            .with_table::<NodeIndex, u8>("uptime")
            .with_table::<Blake3Hash, BTreeSet<NodeIndex>>("uri_to_node")
            .with_table::<NodeIndex, BTreeSet<Blake3Hash>>("node_to_uri")
    }

    /// Query Metadata Table
    fn get_metadata(&self, key: &lightning_types::Metadata) -> Option<Value>;

    /// Get the state root hash.
    fn get_state_root<M>(&self) -> Result<StateRootHash>
    where
        M: MerklizeProvider<Storage = Self::Backend, Serde = DefaultSerdeBackend>;

    /// Get a state proof for a given key.
    fn get_state_proof<M>(&self, key: StateProofKey) -> Result<(Option<StateProofValue>, M::Proof)>
    where
        M: MerklizeProvider<Storage = Self::Backend, Serde = DefaultSerdeBackend>;

    /// Query Account Table
    /// Returns information about an account.
    fn get_account_info<V>(
        &self,
        address: &EthAddress,
        selector: impl FnOnce(AccountInfo) -> V,
    ) -> Option<V>;

    /// Query Client Table
    fn client_key_to_account_key(&self, pub_key: &ClientPublicKey) -> Option<EthAddress>;

    /// Query Node Table
    /// Returns information about a single node.
    fn get_node_info<V>(&self, node: &NodeIndex, selector: impl FnOnce(NodeInfo) -> V)
    -> Option<V>;

    /// Returns an Iterator to Node Table
    fn get_node_table_iter<V>(&self, closure: impl FnOnce(KeyIterator<NodeIndex>) -> V) -> V;

    /// Query Pub Key to Node Index Table
    fn pubkey_to_index(&self, pub_key: &NodePublicKey) -> Option<NodeIndex>;

    /// Query Committee Table
    fn get_committe_info<V>(
        &self,
        epoch: &Epoch,
        selector: impl FnOnce(Committee) -> V,
    ) -> Option<V>;

    /// Query Services Table
    /// Returns the service information for a given [`ServiceId`]
    fn get_service_info(&self, id: &ServiceId) -> Option<Service>;

    /// Query Params Table
    /// Returns the passed in protocol parameter
    fn get_protocol_param(&self, param: &ProtocolParams) -> Option<u128>;

    /// Query Current Epoch Served Table
    fn get_current_epoch_served(&self, node: &NodeIndex) -> Option<NodeServed>;

    /// Query Reputation Measurements Table
    /// Returns the reported reputation measurements for a node.
    fn get_reputation_measurements(
        &self,
        node: &NodeIndex,
    ) -> Option<Vec<ReportedReputationMeasurements>>;

    /// Query Latencies Table
    fn get_latencies(&self, nodes: &(NodeIndex, NodeIndex)) -> Option<Duration>;

    /// Returns an Iterator to Latencies Table
    fn get_latencies_iter<V>(
        &self,
        closure: impl FnOnce(KeyIterator<(NodeIndex, NodeIndex)>) -> V,
    ) -> V;

    /// Query Reputation Scores Table
    /// Returns the global reputation of a node.
    fn get_reputation_score(&self, node: &NodeIndex) -> Option<u8>;

    /// Query Total Served Table
    /// Returns total served for all commodities from the state for a given epoch
    fn get_total_served(&self, epoch: &Epoch) -> Option<TotalServed>;

    /// Checks if an transaction digest has been executed this epoch.
    fn has_executed_digest(&self, digest: TxHash) -> bool;

    /// Get Node's Public Key based on the Node's Index
    fn index_to_pubkey(&self, node_index: &NodeIndex) -> Option<NodePublicKey>;

    /// Simulate Transaction
    fn simulate_txn(&self, txn: TransactionRequest) -> TransactionResponse;

    /// Returns the uptime for a node from the past epoch.
    fn get_node_uptime(&self, node_index: &NodeIndex) -> Option<u8>;

    /// Returns nodes that are providing the content addressed by the cid.
    fn get_uri_providers(&self, uri: &Blake3Hash) -> Option<BTreeSet<NodeIndex>>;

    /// Returns the node's content registry.
    fn get_content_registry(&self, node_index: &NodeIndex) -> Option<BTreeSet<Blake3Hash>>;
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
