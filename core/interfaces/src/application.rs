use std::collections::{BTreeSet, HashMap};
use std::time::Duration;

use affair::Socket;
use anyhow::Result;
use atomo::KeyIterator;
use autometrics::autometrics;
use fleek_crypto::{ClientPublicKey, EthAddress, NodePublicKey};
use lightning_types::{
    AccountInfo,
    Blake3Hash,
    Committee,
    Metadata,
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
    /// Returns information about an account.
    fn get_account_info<V>(
        &self,
        address: &EthAddress,
        selector: impl FnOnce(AccountInfo) -> V,
    ) -> Option<V>;

    /// Query Client Table
    fn client_key_to_account_key(self, pub_key: &ClientPublicKey) -> Option<EthAddress>;

    /// Query Node Table
    /// Returns information about a single node.
    fn get_node_info<V>(&self, node: &NodeIndex, selector: impl FnOnce(NodeInfo) -> V)
    -> Option<V>;

    /// Returns an Iterator to Node Table
    fn get_node_table_iter<V>(&self, closure: impl FnOnce(KeyIterator<NodeIndex>) -> V) -> V;

    /// Query Pub Key to Node Index Table
    fn pubkey_to_index(&self, pub_key: &NodePublicKey) -> Option<NodeIndex>;

    /// Query Committe Table
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
    fn get_cid_providers(&self, cid: &Blake3Hash) -> Option<BTreeSet<NodeIndex>>;

    /// Returns the node's content registry.
    fn get_content_registry(&self, node_index: &NodeIndex) -> Option<BTreeSet<Blake3Hash>>;
}

pub trait QueryRunnerExt: SyncQueryRunnerInterface {
    /// Returns the chain id
    fn get_chain_id(&self) -> u32 {
        match self.get_metadata(&Metadata::ChainId) {
            Some(Value::ChainId(id)) => id,
            _ => 0,
        }
    }

    /// Returns the committee members of the current epoch
    #[autometrics]
    fn get_committee_members(&self) -> Vec<NodePublicKey> {
        self.get_committee_members_by_index()
            .into_iter()
            .filter_map(|node_index| self.index_to_pubkey(&node_index))
            .collect()
    }

    /// Returns the committee members of the current epoch by NodeIndex
    fn get_committee_members_by_index(&self) -> Vec<NodeIndex> {
        let epoch = self.get_current_epoch();
        self.get_committe_info(&epoch, |c| c.members)
            .unwrap_or_default()
    }

    /// Get Current Epoch
    /// Returns just the current epoch
    fn get_current_epoch(&self) -> Epoch {
        match self.get_metadata(&Metadata::Epoch) {
            Some(Value::Epoch(epoch)) => epoch,
            _ => 0,
        }
    }

    /// Get Current Epoch Info
    /// Returns all the information on the current epoch that Narwhal needs to run
    fn get_epoch_info(&self) -> EpochInfo {
        let epoch = self.get_current_epoch();
        // look up current committee
        let committee = self.get_committe_info(&epoch, |c| c).unwrap_or_default();
        EpochInfo {
            committee: committee
                .members
                .iter()
                .filter_map(|member| self.get_node_info::<NodeInfo>(member, |n| n))
                .collect(),
            epoch,
            epoch_end: committee.epoch_end_timestamp,
        }
    }

    /// Return all latencies measurements for the current epoch.
    fn get_current_latencies(&self) -> HashMap<(NodePublicKey, NodePublicKey), Duration> {
        self.get_latencies_iter::<HashMap<(NodePublicKey, NodePublicKey), Duration>>(
            |latencies| -> HashMap<(NodePublicKey, NodePublicKey), Duration> {
                latencies
                    .filter_map(|nodes| self.get_latencies(&nodes).map(|latency| (nodes, latency)))
                    .filter_map(|((index_lhs, index_rhs), latency)| {
                        let node_lhs =
                            self.get_node_info::<NodePublicKey>(&index_lhs, |n| n.public_key);
                        let node_rhs =
                            self.get_node_info::<NodePublicKey>(&index_rhs, |n| n.public_key);
                        match (node_lhs, node_rhs) {
                            (Some(node_lhs), Some(node_rhs)) => {
                                Some(((node_lhs, node_rhs), latency))
                            },
                            _ => None,
                        }
                    })
                    .collect()
            },
        )
    }

    /// Returns the node info of the genesis committee members
    fn get_genesis_committee(&self) -> Vec<(NodeIndex, NodeInfo)> {
        match self.get_metadata(&Metadata::GenesisCommittee) {
            Some(Value::GenesisCommittee(committee)) => committee
                .iter()
                .filter_map(|node_index| {
                    self.get_node_info::<NodeInfo>(node_index, |n| n)
                        .map(|node_info| (*node_index, node_info))
                })
                .collect(),
            _ => {
                // unreachable seeded at genesis
                Vec::new()
            },
        }
    }

    /// Returns last executed block hash. [0;32] is genesis
    fn get_last_block(&self) -> [u8; 32] {
        match self.get_metadata(&Metadata::LastBlockHash) {
            Some(Value::Hash(hash)) => hash,
            _ => [0; 32],
        }
    }

    /// Returns a full copy of the entire node-registry,
    /// Paging Params - filtering nodes that are still a valid node and have enough stake; Takes
    /// from starting index and specified amount.
    fn get_node_registry(&self, paging: Option<PagingParams>) -> Vec<NodeInfoWithIndex> {
        let staking_amount = self.get_staking_amount().into();

        self.get_node_table_iter::<Vec<NodeInfoWithIndex>>(|nodes| -> Vec<NodeInfoWithIndex> {
            let nodes = nodes.map(|index| NodeInfoWithIndex {
                index,
                info: self.get_node_info::<NodeInfo>(&index, |n| n).unwrap(),
            });
            match paging {
                None => nodes
                    .filter(|node| node.info.stake.staked >= staking_amount)
                    .collect(),
                Some(PagingParams {
                    ignore_stake,
                    limit,
                    start,
                }) => {
                    let mut nodes = nodes
                        .filter(|node| ignore_stake || node.info.stake.staked >= staking_amount)
                        .collect::<Vec<NodeInfoWithIndex>>();

                    nodes.sort_by_key(|info| info.index);

                    nodes
                        .into_iter()
                        .filter(|info| info.index >= start)
                        .take(limit)
                        .collect()
                },
            }
        })
    }

    /// Returns the amount that is required to be a valid node in the network.
    fn get_staking_amount(&self) -> u128 {
        self.get_protocol_param(&ProtocolParams::MinimumNodeStake)
            .unwrap_or(0)
    }

    /// Returns true if the node is a valid node in the network, with enough stake.
    fn is_valid_node(&self, id: &NodePublicKey) -> bool {
        let minimum_stake_amount = self.get_staking_amount().into();
        self.pubkey_to_index(id).is_some_and(|node_idx| {
            self.get_node_info(&node_idx, |n| n.stake.staked)
                .is_some_and(|node_stake| node_stake >= minimum_stake_amount)
        })
    }
}

impl<T: SyncQueryRunnerInterface> QueryRunnerExt for T {}

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
