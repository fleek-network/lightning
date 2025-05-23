//! The data types used in the application state
use std::borrow::Cow;
use std::fmt::Display;
use std::net::IpAddr;
use std::time::Duration;

use anyhow::anyhow;
use fleek_crypto::{ConsensusPublicKey, EthAddress, NodePublicKey};
use hp_fixed::unsigned::HpUfixed;
use ink_quill::TranscriptBuilderInput;
use multiaddr::Multiaddr;
use num_derive::FromPrimitive;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use sha3::{Digest, Sha3_256};

use super::ReputationMeasurements;
use crate::NodeRegistryChanges;

/// The Id of a Service
pub type ServiceId = u32;

/// Application epoch number
pub type Epoch = u64;

/// Application epoch era.
pub type EpochEra = u64;

/// A nodes index
pub type NodeIndex = u32;

#[derive(Serialize, Deserialize, Hash, Debug, Clone, Eq, PartialEq, schemars::JsonSchema)]
pub enum Tokens {
    USDC,
    FLK,
}

impl Tokens {
    pub fn address(&self) -> EthAddress {
        // todo!(n)
        match self {
            Tokens::USDC => EthAddress([0; 20]),
            Tokens::FLK => EthAddress([1; 20]),
        }
    }
}

#[derive(Debug, PartialEq, Eq, Serialize, Deserialize, Clone, Default, schemars::JsonSchema)]
pub struct NodeServed {
    pub served: CommodityServed,
    pub stables_revenue: HpUfixed<6>,
}

#[derive(Debug, PartialEq, Eq, Serialize, Deserialize, Clone, Default, schemars::JsonSchema)]
pub struct TotalServed {
    pub served: CommodityServed,
    pub reward_pool: HpUfixed<6>,
}

pub type ServiceRevenue = HpUfixed<6>;

/// This is commodity served by each of the commodity types
pub type CommodityServed = Vec<u128>;

/// This is commodities served by different services in Fleek Network.
/// C-like enums used here to future proof for state, if we add more commodity types
#[derive(
    Clone,
    Copy,
    Debug,
    PartialEq,
    Eq,
    Hash,
    Serialize,
    Deserialize,
    FromPrimitive,
    schemars::JsonSchema,
)]
#[repr(u8)]
pub enum CommodityTypes {
    Bandwidth = 0,
    Compute = 1,
    Gpu = 2,
}

#[derive(Clone, Debug, Hash, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ReportedReputationMeasurements {
    pub reporting_node: NodeIndex,
    pub measurements: ReputationMeasurements,
}

/// Metadata, state stored in the blockchain that applies to the current block
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, schemars::JsonSchema)]
pub enum Metadata {
    ChainId,
    Epoch,
    BlockNumber,
    SupplyYearStart,
    TotalSupply,
    ProtocolFundAddress,
    NextNodeIndex,
    GovernanceAddress,
    LastEpochHash,
    LastBlockHash,
    GenesisCommittee,
    SubDagIndex,
    SubDagRound,
    CommitteeSelectionBeaconPhase,
    EpochEra,
    WithdrawId,
    TimeInterval,
}

/// The Value enum is a data type used to represent values in a key-value pair for a metadata table
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub enum Value {
    ChainId(u32),
    Epoch(u64),
    BlockNumber(u64),
    String(String),
    HpUfixed(HpUfixed<18>),
    AccountPublicKey(EthAddress),
    NextNodeIndex(u32),
    Hash([u8; 32]),
    GenesisCommittee(Vec<NodeIndex>),
    SubDagIndex(u64),
    SubDagRound(u64),
    BlockRange(u64, u64),
    CommitteeSelectionBeaconPhase(CommitteeSelectionBeaconPhase),
    EpochEra(u64),
    WithdrawId(u64),
    TimeInterval(u64),
}

impl Value {
    pub fn maybe_hash(self) -> Option<[u8; 32]> {
        match self {
            Value::Hash(hash) => Some(hash),
            _ => None,
        }
    }
}

/// Block number.
pub type BlockNumber = u64;

/// Committee selection beacon round number.
pub type CommitteeSelectionBeaconRound = u64;

/// Committee selection beacon commit.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(transparent)]
pub struct CommitteeSelectionBeaconCommit {
    pub hash: [u8; 32],
}

impl CommitteeSelectionBeaconCommit {
    pub fn new(hash: [u8; 32]) -> Self {
        Self { hash }
    }

    pub fn build(
        epoch: Epoch,
        round: CommitteeSelectionBeaconRound,
        reveal: CommitteeSelectionBeaconReveal,
    ) -> Self {
        let hash = Sha3_256::digest(
            [
                epoch.to_be_bytes().as_slice(),
                round.to_be_bytes().as_slice(),
                &reveal,
            ]
            .concat(),
        )
        .into();

        Self::new(hash)
    }
}

impl From<[u8; 32]> for CommitteeSelectionBeaconCommit {
    fn from(hash: [u8; 32]) -> Self {
        Self::new(hash)
    }
}

impl From<CommitteeSelectionBeaconCommit> for [u8; 32] {
    fn from(commit: CommitteeSelectionBeaconCommit) -> Self {
        commit.hash
    }
}

/// Committee selection beacon reveal.
pub type CommitteeSelectionBeaconReveal = [u8; 32];

/// Phase of the committee selection beacon.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, schemars::JsonSchema)]
pub enum CommitteeSelectionBeaconPhase {
    Commit((Epoch, CommitteeSelectionBeaconRound)),
    Reveal((Epoch, CommitteeSelectionBeaconRound)),
}

impl CommitteeSelectionBeaconPhase {
    pub fn get_epoch(&self) -> Epoch {
        match self {
            CommitteeSelectionBeaconPhase::Commit((epoch, _)) => *epoch,
            CommitteeSelectionBeaconPhase::Reveal((epoch, _)) => *epoch,
        }
    }

    pub fn get_round(&self) -> CommitteeSelectionBeaconRound {
        match self {
            CommitteeSelectionBeaconPhase::Commit((_, round)) => *round,
            CommitteeSelectionBeaconPhase::Reveal((_, round)) => *round,
        }
    }
}

/// Indicates the participation status of a node.
#[rustfmt::skip]
#[derive(
    Debug,
    Hash,
    PartialEq,
    PartialOrd,
    Ord,
    Eq,
    Serialize,
    Deserialize,
    Clone,
    schemars::JsonSchema,
)]
pub enum Participation {
    True,
    False,
    OptedIn,
    OptedOut,
}

/// Adjustable parameters that are stored in the blockchain
#[rustfmt::skip]
#[derive(
    Clone,
    PartialEq,
    Eq,
    Hash,
    Serialize,
    Deserialize,
    Debug,
    schemars::JsonSchema
)]

#[repr(u8)]
pub enum ProtocolParamKey {
    /// The time in milliseconds that an epoch lasts for. Genesis 24 hours(86400)
    EpochTime = 0,
    /// The size of the committee
    CommitteeSize = 1,
    /// The amount of nodes allowed to participate in the network
    NodeCount = 2,
    /// The min FLK a node has to stake to participate in the network
    MinimumNodeStake = 3,
    /// The time in epochs a node has to be staked to participate in the network
    EligibilityTime = 4,
    /// The time in epochs a node has to wait to withdraw after unstaking
    LockTime = 5,
    /// The percentage of the reward pool the protocol gets
    ProtocolShare = 6,
    /// The percentage of the reward pool goes to edge nodes
    NodeShare = 7,
    /// The percentage of the reward pool goes to edge nodes
    ServiceBuilderShare = 8,
    /// The maximum target inflation rate in a year
    MaxInflation = 9,
    /// The max multiplier on rewards for locking
    MaxBoost = 10,
    /// The max amount of time tokens can be locked
    MaxStakeLockTime = 11,
    /// Minimum number of reported measurements that have to be available for a node. If less
    /// measurements have been reported, no reputation score will be computed in that epoch.
    MinNumMeasurements = 12,
    /// The public key corresponding to the secret key that is shared among the SGX enclaves
    SGXSharedPubKey = 13,
    /// The number of epochs per year
    EpochsPerYear = 14,
    /// The ping timeout for node reputation.
    ReputationPingTimeout = 15,
    /// Topology clustering target k value.
    TopologyTargetK = 16,
    /// The minimum number of nodes to run the topology algorithm.
    TopologyMinNodes = 17,
    /// The committee selection beacon commit phase duration in blocks
    CommitteeSelectionBeaconCommitPhaseDuration = 18,
    /// The committee selection beacon reveal phase duration in blocks
    CommitteeSelectionBeaconRevealPhaseDuration = 19,
    /// The slash amount for non-revealing nodes in the committee selection beacon process.
    CommitteeSelectionBeaconNonRevealSlashAmount = 20,
    TotalTimeIntervals = 21,
}

/// The Value enum is a data type used to represent values in a key-value pair for a metadata table
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Hash, schemars::JsonSchema)]
pub enum ProtocolParamValue {
    EpochTime(u64),
    EpochsPerYear(u64),
    CommitteeSize(u64),
    NodeCount(u64),
    MinimumNodeStake(u64),
    EligibilityTime(u64),
    LockTime(u64),
    ProtocolShare(u16),
    NodeShare(u16),
    ServiceBuilderShare(u16),
    MaxInflation(u16),
    MaxBoost(u16),
    MaxStakeLockTime(u64),
    MinNumMeasurements(u64),
    SGXSharedPubKey(String),
    ReputationPingTimeout(Duration),
    TopologyTargetK(usize),
    TopologyMinNodes(usize),
    CommitteeSelectionBeaconCommitPhaseDuration(u64),
    CommitteeSelectionBeaconRevealPhaseDuration(u64),
    CommitteeSelectionBeaconNonRevealSlashAmount(u64),
    TotalTimeIntervals(u64),
}

impl ProtocolParamValue {
    pub fn get_bytes(&self) -> Cow<'_, [u8]> {
        match self {
            ProtocolParamValue::EpochTime(i) => Cow::Owned(i.to_le_bytes().to_vec()),
            ProtocolParamValue::EpochsPerYear(i) => Cow::Owned(i.to_le_bytes().to_vec()),
            ProtocolParamValue::CommitteeSize(i) => Cow::Owned(i.to_le_bytes().to_vec()),
            ProtocolParamValue::NodeCount(i) => Cow::Owned(i.to_le_bytes().to_vec()),
            ProtocolParamValue::MinimumNodeStake(i) => Cow::Owned(i.to_le_bytes().to_vec()),
            ProtocolParamValue::EligibilityTime(i) => Cow::Owned(i.to_le_bytes().to_vec()),
            ProtocolParamValue::LockTime(i) => Cow::Owned(i.to_le_bytes().to_vec()),
            ProtocolParamValue::ProtocolShare(i) => Cow::Owned(i.to_le_bytes().to_vec()),
            ProtocolParamValue::NodeShare(i) => Cow::Owned(i.to_le_bytes().to_vec()),
            ProtocolParamValue::ServiceBuilderShare(i) => Cow::Owned(i.to_le_bytes().to_vec()),
            ProtocolParamValue::MaxInflation(i) => Cow::Owned(i.to_le_bytes().to_vec()),
            ProtocolParamValue::MaxBoost(i) => Cow::Owned(i.to_le_bytes().to_vec()),
            ProtocolParamValue::MaxStakeLockTime(i) => Cow::Owned(i.to_le_bytes().to_vec()),
            ProtocolParamValue::MinNumMeasurements(i) => Cow::Owned(i.to_le_bytes().to_vec()),
            ProtocolParamValue::SGXSharedPubKey(s) => Cow::Borrowed(s.as_bytes()),
            ProtocolParamValue::ReputationPingTimeout(d) => {
                Cow::Owned(d.as_millis().to_le_bytes().to_vec())
            },
            ProtocolParamValue::TopologyTargetK(i) => Cow::Owned(i.to_le_bytes().to_vec()),
            ProtocolParamValue::TopologyMinNodes(i) => Cow::Owned(i.to_le_bytes().to_vec()),
            ProtocolParamValue::CommitteeSelectionBeaconCommitPhaseDuration(i) => {
                Cow::Owned(i.to_le_bytes().to_vec())
            },
            ProtocolParamValue::CommitteeSelectionBeaconRevealPhaseDuration(i) => {
                Cow::Owned(i.to_le_bytes().to_vec())
            },
            ProtocolParamValue::CommitteeSelectionBeaconNonRevealSlashAmount(i) => {
                Cow::Owned(i.to_le_bytes().to_vec())
            },
            ProtocolParamValue::TotalTimeIntervals(i) => Cow::Owned(i.to_le_bytes().to_vec()),
        }
    }
}

#[rustfmt::skip]
#[derive(
    Debug,
    Hash,
    PartialEq,
    PartialOrd,
    Ord,
    Eq,
    Serialize,
    Deserialize,
    Clone,
    schemars::JsonSchema,
)]
pub struct NodeInfo {
    /// The owner of this node
    pub owner: EthAddress,
    /// Public key that is used for fast communication signatures for this node.
    pub public_key: NodePublicKey,
    /// The BLS public key of the node which is used for our BFT DAG consensus
    /// multi signatures.
    pub consensus_key: ConsensusPublicKey,
    /// The epoch that this node has been staked since,
    pub staked_since: Epoch,
    /// The amount of stake by the node.
    pub stake: Staking,
    /// The nodes primary domain
    pub domain: IpAddr,
    /// The node workers domain
    pub worker_domain: IpAddr,
    /// Open ports for this node
    pub ports: NodePorts,
    /// The public key of the nodes narwhal worker
    pub worker_public_key: NodePublicKey,
    /// The participation status of the node
    pub participation: Participation,
    /// The nonce of the node. Added to each transaction before signed to prevent replays and
    /// enforce ordering
    pub nonce: u64,
}

#[derive(Debug, Serialize, Deserialize, Clone, schemars::JsonSchema)]
pub struct NodeInfoWithIndex {
    pub index: NodeIndex,
    pub info: NodeInfo,
}

#[rustfmt::skip]
#[derive(
    Debug,
    Hash,
    PartialEq,
    PartialOrd,
    Ord,
    Eq,
    Serialize,
    Deserialize,
    Clone,
    schemars::JsonSchema,
)]
/// The ports a node has open for its processes
pub struct NodePorts {
    pub primary: u16,
    pub worker: u16,
    pub mempool: u16,
    pub rpc: u16,
    pub pool: u16,
    pub pinger: u16,
    pub handshake: HandshakePorts,
}

impl Default for NodePorts {
    fn default() -> Self {
        Self {
            pool: 4300,
            primary: 4310,
            worker: 4311,
            mempool: 4210,
            handshake: Default::default(),
            rpc: 4240,
            pinger: 4350,
        }
    }
}

impl Display for NodePorts {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Node Ports:
  Pool: {}
  Primary: {}
  Worker: {}
  Mempool: {}
  RPC: {}
  Pinger: {}
  Handshake:
    HTTP: {}
    WebRTC: {}
    WebTransport: {}",
            self.pool,
            self.primary,
            self.worker,
            self.mempool,
            self.rpc,
            self.pinger,
            self.handshake.http,
            self.handshake.webrtc,
            self.handshake.webtransport
        )
    }
}

/// The ports a node has open for the handshake server
#[rustfmt::skip]
#[derive(
    Debug,
    Hash,
    PartialEq,
    PartialOrd,
    Ord,
    Eq,
    Serialize,
    Deserialize,
    Clone,
    schemars::JsonSchema,
)]
pub struct HandshakePorts {
    pub http: u16,
    pub webrtc: u16,
    pub webtransport: u16,
}

impl Default for HandshakePorts {
    fn default() -> Self {
        Self {
            http: 4220,
            webrtc: 4320,
            webtransport: 4321,
        }
    }
}

/// Struct that stores the information about the stake of amount of a node.
#[derive(
    Debug,
    Hash,
    PartialEq,
    PartialOrd,
    Ord,
    Eq,
    Serialize,
    Deserialize,
    Clone,
    Default,
    schemars::JsonSchema,
)]
pub struct Staking {
    /// How much FLK that is currently staked
    pub staked: HpUfixed<18>,
    /// The epoch until all stakes are locked for boosting rewards
    pub stake_locked_until: u64,
    /// How much FLK is locked pending withdraw
    pub locked: HpUfixed<18>,
    /// The epoch the locked FLK is eligible to be withdrawn
    pub locked_until: u64,
}

#[derive(Debug, Hash, PartialEq, PartialOrd, Ord, Eq, Serialize, Deserialize, Clone)]
pub struct Worker {
    /// The public key of the worker
    pub public_key: NodePublicKey,
    /// The workers internet address
    pub address: Multiaddr,
    /// The address to the workers mempool
    pub mempool: Multiaddr,
}

/// Placeholder
/// Information about the services
#[derive(Clone, Copy, Debug, Serialize, Deserialize, Hash, Eq, PartialEq, schemars::JsonSchema)]
pub struct Service {
    /// the owner address that deploys the service and also recieves reward share
    pub owner: EthAddress,
    // TODO: can there be multiple types of commodity per service
    /// the commodity that service is going to serve
    pub commodity_type: CommodityTypes,
    /// TODO: List of circuits to prove a node should be slashed
    pub slashing: (),
}

#[derive(
    Debug,
    Hash,
    PartialEq,
    PartialOrd,
    Ord,
    Eq,
    Serialize,
    Deserialize,
    Clone,
    Default,
    schemars::JsonSchema,
)]
pub struct Committee {
    /// The members of the committee
    pub members: Vec<NodeIndex>,
    /// The members of the committee that already submitted the epoch change txn
    pub ready_to_change: Vec<NodeIndex>,
    /// The members of the committee that already submitted the commit phase timeout txn
    pub signalled_commit_phase_timeout: Vec<NodeIndex>,
    /// The members of the committee that already submitted the reveal phase timeout txn
    pub signalled_reveal_phase_timeout: Vec<NodeIndex>,
    /// The timestamp when the epoch will end (approximetaly)
    pub epoch_end_timestamp: u64,
    /// The timestamp when nodes will send the epoch change transaction
    pub epoch_transition_timestamp: u64,
    /// The nodes with sufficient stake
    pub active_node_set: Vec<NodeIndex>,
    /// Changes to the node registry (I think this is only used for testing)
    pub node_registry_changes: NodeRegistryChanges,
}

impl TranscriptBuilderInput for Service {
    const TYPE: &'static str = "service";

    fn to_transcript_builder_input(&self) -> Vec<u8> {
        self.commodity_type.to_transcript_builder_input()
        // todo: check if implementation needs to change when slashing is implemented
    }
}

impl TranscriptBuilderInput for Tokens {
    const TYPE: &'static str = "Tokens";

    fn to_transcript_builder_input(&self) -> Vec<u8> {
        match self {
            Tokens::USDC => b"USDC".to_vec(),
            Tokens::FLK => b"FLK".to_vec(),
        }
    }
}

impl TranscriptBuilderInput for CommodityTypes {
    const TYPE: &'static str = "commodity_types";

    fn to_transcript_builder_input(&self) -> Vec<u8> {
        match self {
            CommodityTypes::Bandwidth => b"Bandwidth".to_vec(),
            CommodityTypes::Compute => b"Compute".to_vec(),
            CommodityTypes::Gpu => b"Gpu".to_vec(),
        }
    }
}

impl TryFrom<String> for Tokens {
    type Error = anyhow::Error;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        match value.as_str() {
            "flk" | "FLK" => Ok(Tokens::FLK),
            "usdc" | "USDC" => Ok(Tokens::USDC),
            _ => Err(anyhow!("Invalid token: {value}")),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Hash, Deserialize, Serialize, JsonSchema)]
pub struct Job {
    /// The hash of the job.
    pub hash: [u8; 32],
    /// Information about the job for execution purposes.
    pub info: JobInfo,
    /// The status of the most recent execution of a job.
    pub status: Option<JobStatus>,
    /// The node to which this job was assigned.
    pub assignee: Option<NodeIndex>,
}

#[derive(Clone, Debug, Eq, PartialEq, Hash, Deserialize, Serialize, JsonSchema)]
pub struct JobInfo {
    /// The frequency in which this job should be performed.
    pub frequency: u32,
    /// Amount prepaid.
    pub amount: u32,
    /// The service that will execute the function.
    pub service: ServiceId,
    /// The arguments for the job.
    pub arguments: Box<[u8]>,
}

#[derive(Clone, Debug, Eq, PartialEq, Hash, Deserialize, Serialize, JsonSchema)]
pub struct JobStatus {
    /// Timestamp of the most recent execution.
    pub last_run: u64,
    /// Indicates whether the last execution was successful.
    pub success: bool,
    /// Records any error message.
    pub message: Option<String>,
}
