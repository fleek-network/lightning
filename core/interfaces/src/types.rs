use std::{collections::BTreeMap, time::Duration};

use blake3_tree::blake3::derive_key;
use fleek_crypto::{
    EthAddress, NodeNetworkingPublicKey, NodePublicKey, PublicKey, TransactionSender,
    TransactionSignature,
};
use hp_float::unsigned::HpUfloat;
use multiaddr::Multiaddr;
use num_derive::FromPrimitive;
use random_oracle::{RandomOracle, RandomOracleInput};
use serde::{Deserialize, Serialize};

use crate::{common::ToDigest, pod::DeliveryAcknowledgment};

const FN_TXN_PAYLOAD_DOMAIN: &str = "fleek_network_txn_payload";

/// Unix time stamp in second.
pub type UnixTs = u64;

/// Application epoch number
pub type Epoch = u64;

/// The Id of a Service
pub type ServiceId = u32;

/// A block of transactions, which is a list of update requests each signed by a user,
/// the block is the atomic view into the network, meaning that queries do not view
/// the intermediary state within a block, but only have the view to the latest executed
/// block.
#[derive(Debug)]
pub struct Block {
    pub transactions: Vec<UpdateRequest>,
}

#[derive(Serialize, Deserialize, Hash, Debug, Clone)]
pub enum Tokens {
    USDC,
    FLK,
}

/// The Value enum is a data type used to represent values in a key-value pair for a metadata table
#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum Value {
    Epoch(u64),
    String(String),
    HpUfloat(HpUfloat<18>),
    AccountPublicKey(EthAddress),
}

/// This is commodities served by different services in Fleek Network.
/// C-like enums used here to future proof for state, if we add more commodity types
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, FromPrimitive)]
#[repr(u8)]
pub enum CommodityTypes {
    Bandwidth = 0,
    Compute = 1,
    Gpu = 2,
}

/// This is commodity served by each of the commodity types
pub type CommodityServed = Vec<u128>;

#[derive(Debug, PartialEq, Serialize, Deserialize, Clone, Default)]
pub struct TotalServed {
    pub served: CommodityServed,
    pub reward_pool: HpUfloat<6>,
}

/// Placeholder
/// Information about the services
#[derive(Clone, Copy, Debug, Serialize, Deserialize, Hash)]
pub struct Service {
    pub commodity_type: CommodityTypes,
    /// TODO: List of circuits to prove a node should be slashed
    pub slashing: (),
}

/// An update transaction, sent from users to the consensus to migrate the application
/// from one state to the next state.
#[derive(Debug, Hash, Clone, Serialize, Deserialize)]
pub struct UpdateRequest {
    /// The sender of the transaction.
    pub sender: TransactionSender,
    /// The signature by the user signing this payload.
    pub signature: TransactionSignature,
    /// The payload of an update request, which contains a counter (nonce), and
    /// the transition function itself.
    pub payload: UpdatePayload,
}

/// The payload data of an update request.
#[derive(Debug, Hash, Clone, Serialize, Deserialize)]
pub struct UpdatePayload {
    /// The counter or nonce of this request.
    pub nonce: u64,
    /// The transition function (and parameters) for this update request.
    pub method: UpdateMethod,
}

/// All of the update functions in our logic, along their parameters.
#[derive(Debug, Hash, Clone, Serialize, Deserialize)]
pub enum UpdateMethod {
    /// The main function of the application layer. After aggregating ProofOfAcknowledgements a
    /// node will submit this     transaction to get paid.
    /// Revisit the naming of this transaction.
    SubmitDeliveryAcknowledgmentAggregation {
        /// How much of the commodity was served
        commodity: u128,
        /// The service id of the service this was provided through(CDN, compute, ect.)
        service_id: u32,
        /// The PoD of delivery in bytes
        proofs: Vec<DeliveryAcknowledgment>,
        /// Optional metadata to provide information additional information about this batch
        metadata: Option<Vec<u8>>,
    },
    /// Withdraw tokens from the network back to the L2
    Withdraw {
        /// The amount to withdrawl
        amount: HpUfloat<18>,
        /// Which token to withdrawl
        token: Tokens,
        /// The address to recieve these tokens on the L2
        receiving_address: EthAddress,
    },
    /// Submit of PoC from the bridge on the L2 to get the tokens in network
    Deposit {
        /// The proof of the bridge recieved from the L2,
        proof: ProofOfConsensus,
        /// Which token was bridged
        token: Tokens,
        /// Amount bridged
        amount: HpUfloat<18>,
    },
    /// Stake FLK in network
    Stake {
        /// Amount to stake
        amount: HpUfloat<18>,
        /// Node Public Key
        node_public_key: NodePublicKey,
        /// Node networking key for narwhal
        node_network_key: Option<NodeNetworkingPublicKey>,
        /// Nodes primary internet address
        node_domain: Option<String>,
        /// Worker public Key
        worker_public_key: Option<NodeNetworkingPublicKey>,
        /// internet address for the worker
        worker_domain: Option<String>,
        /// internet address for workers mempool
        worker_mempool_address: Option<String>,
    },
    /// Lock the current stakes to get boosted inflation rewards
    /// this is different than unstake and withdrawl lock
    StakeLock {
        node: NodePublicKey,
        locked_for: u64,
    },
    /// Unstake FLK, the tokens will be locked for a set amount of
    /// time(ProtocolParameter::LockTime) before they can be withdrawn
    Unstake {
        amount: HpUfloat<18>,
        node: NodePublicKey,
    },
    /// Withdraw tokens from a node after lock period has passed
    /// must be submitted by node owner but optionally they can provide a different public key to
    /// recieve the tokens
    WithdrawUnstaked {
        node: NodePublicKey,
        recipient: Option<EthAddress>,
    },
    /// Sent by committee member to signal he is ready to change epoch
    ChangeEpoch { epoch: Epoch },
    /// Adding a new service to the protocol
    AddService {
        service: Service,
        service_id: ServiceId,
    },
    /// Removing a service from the protocol
    RemoveService {
        /// Service Id of the service to be removed
        service_id: ServiceId,
    },
    /// Provide proof of misbehavior to slash a node
    Slash {
        /// Service id of the service a node misbehaved in
        service_id: ServiceId,
        /// The public key of the node that misbehaved
        node: NodePublicKey,
        /// Zk proof to be provided to the slash circuit
        proof_of_misbehavior: ProofOfMisbehavior,
    },
    SubmitReputationMeasurements {
        measurements: BTreeMap<NodePublicKey, ReputationMeasurements>,
    },
}

#[derive(Clone, Debug, PartialEq, PartialOrd, Hash, Eq, Serialize, Deserialize)]
pub enum TransactionResponse {
    Success(ExecutionData),
    Revert(ExecutionError),
}

#[derive(Clone, Debug, PartialEq, PartialOrd, Hash, Eq, Serialize, Deserialize)]
pub enum ExecutionData {
    None,
    String(String),
    UInt(u128),
    EpochInfo(EpochInfo),
    EpochChange,
}

/// Error type for transaction execution on the application layer
#[derive(Clone, Debug, PartialEq, PartialOrd, Hash, Eq, Serialize, Deserialize)]
pub enum ExecutionError {
    InsufficientBalance,
    InvalidSignature,
    InvalidNonce,
    InvalidProof,
    InvalidInternetAddress,
    InsufficientNodeDetails,
    NoLockedTokens,
    TokensLocked,
    NotNodeOwner,
    NotCommitteeMember,
    NodeDoesNotExist,
    AlreadySignaled,
    NonExistingService,
    OnlyAccountOwner,
    OnlyNode,
    InvalidServiceId,
    InsufficientStakesToLock,
    LockExceededMaxLockTime,
    LockedTokensUnstakeForbidden,
    EpochAlreadyChanged,
    EpochHasNotStarted,
}

/// The account info stored per account on the blockchain
#[derive(Debug, Hash, PartialEq, PartialOrd, Ord, Eq, Serialize, Deserialize, Clone, Default)]
pub struct AccountInfo {
    /// The accounts FLK balance
    pub flk_balance: HpUfloat<18>,
    /// the accounts stable coin balance
    pub stables_balance: HpUfloat<6>,
    /// The accounts stables/bandwidth balance
    pub bandwidth_balance: u128,
    /// The nonce of the account. Added to each transaction before signed to prevent replays and
    /// enforce ordering
    pub nonce: u64,
}

/// Struct that stores
#[derive(Debug, Hash, PartialEq, PartialOrd, Ord, Eq, Serialize, Deserialize, Clone, Default)]
pub struct Staking {
    /// How much FLK that is currently staked
    pub staked: HpUfloat<18>,
    /// The epoch until all stakes are locked for boosting rewards
    pub stake_locked_until: u64,
    /// How much FLK is locked pending withdrawl
    pub locked: HpUfloat<18>,
    /// The epoch the locked FLK is elegible to be withdrawn
    pub locked_until: u64,
}

#[derive(Debug, Hash, PartialEq, PartialOrd, Ord, Eq, Serialize, Deserialize, Clone)]
pub struct NodeInfo {
    /// The owner of this node
    pub owner: EthAddress,
    /// The BLS public key of the node which is used for our BFT DAG consensus
    /// multi signatures.
    pub public_key: NodePublicKey,
    /// Public key that is used for fast communication signatures for this node.
    pub network_key: NodeNetworkingPublicKey,
    /// The epoch that this node has been staked since,
    pub staked_since: Epoch,
    /// The amount of stake by the node.
    pub stake: Staking,
    /// The nodes primary internet address
    pub domain: Multiaddr,
    /// A vec of all of this nodes Narwhal workers
    pub workers: Vec<Worker>,
    /// The nonce of the node. Added to each transaction before signed to prevent replays and
    /// enforce ordering
    pub nonce: u64,
}

#[derive(Debug, Hash, PartialEq, PartialOrd, Ord, Eq, Serialize, Deserialize, Clone)]
pub struct Worker {
    /// The public key of the worker
    pub public_key: NodeNetworkingPublicKey,
    /// The workers internet address
    pub address: Multiaddr,
    /// The address to the workers mempool
    pub mempool: Multiaddr,
}

/// Info on a Narwhal epoch
#[derive(Clone, Debug, PartialEq, PartialOrd, Hash, Eq, Serialize, Deserialize)]
pub struct EpochInfo {
    /// List of committee members
    pub committee: Vec<NodeInfo>,
    /// The current epoch number
    pub epoch: Epoch,
    /// Timestamp when the epoch ends
    pub epoch_end: u64,
}

/// Metadata, state stored in the blockchain that applies to the current block
#[derive(Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Metadata {
    Epoch,
    SupplyYearStart,
    TotalSupply,
    ProtocolFundAddress,
}

/// Adjustable paramaters that are stored in the blockchain
#[derive(Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[repr(u8)]
pub enum ProtocolParams {
    /// The time in seconds that an epoch lasts for. Genesis 24 hours(86400)
    EpochTime = 0,
    /// The size of the committee
    CommitteeSize = 1,
    /// The min FLK a node has to stake to participate in the network
    MinimumNodeStake = 2,
    /// The time in epochs a node has to be staked to participate in the network
    EligibilityTime = 3,
    /// The time in epochs a node has to wait to withdraw after unstaking
    LockTime = 4,
    /// The percentage of the reward pool the protocol gets
    ProtocolShare = 5,
    /// The percentage of the reward pool the shared amongst the committe of validators
    ValidatorShare = 6,
    /// The percentage of the FLK emissions per unit that goes to service nodes
    NodeShare = 7,
    /// The maximum targed inflation rate in a year
    MaxInflation = 8,
    /// The amount of FLK minted per GB they consume.
    ConsumerRebate = 9,
    /// The max multiplier on rewards for locking
    MaxBoost = 10,
    /// The max amount of time tokens can be locked
    MaxLockTime = 11,
}

/// The physical address of a node where it can be reached, the port numbers are
/// omitted since each node is responsible to open the standard port numbers for
/// different endpoints and it is unfeasible for us to try to keep a record of
/// this information.
///
/// For example one case to make about this decision is the fact that endpoints
/// are part of an implementation detail and we don't really want that level of
/// book keeping about which parts of a healthy system a node is running, due to
/// the fact that different versions of the software might expose different endpoints
/// a node might offer metrics endpoint publicly while another node might close
/// this port. So it is up to the implementation to pick these ports for different
/// reasons and a node runner that is running an actual node on the mainnet should
/// not modify these default port numbers. Just like how 80 is the port for HTTP,
/// and 443 is the port for SSL traffic, we should chose our numbers and stick
/// with them.
#[derive(Debug, Hash, PartialEq, PartialOrd, Ord, Eq, Serialize, Deserialize, Clone)]
pub enum InternetAddress {
    Ipv4([u8; 4]),
    Ipv6([u8; 16]),
}

impl ToDigest for UpdatePayload {
    /// Computes the hash of this update payload and returns a 32-byte hash
    /// that can be signed by the user.
    ///
    /// # Safety
    ///
    /// This function must take all of the data into account, including the
    /// nonce, the name of all of the update method names along with the value
    /// for all of the parameters.
    fn to_digest(&self) -> [u8; 32] {
        let mut random_oracle =
            RandomOracle::empty(FN_TXN_PAYLOAD_DOMAIN).with("nonce", &self.nonce);

        // insert method fields
        match &self.method {
            UpdateMethod::Deposit {
                proof: _,
                token,
                amount,
            } => {
                random_oracle = random_oracle
                    .with("transaction_name", &"deposit")
                    .with_prefix("input".to_owned())
                    .with("token", token)
                    .with("amount", amount);
                //.with("method.proof", proof);
            },

            UpdateMethod::SubmitDeliveryAcknowledgmentAggregation {
                commodity,
                service_id,
                proofs: _,
                metadata,
            } => {
                random_oracle = random_oracle
                    .with(
                        "transaction_name",
                        &"submit_delivery_acknowledgment_aggregation",
                    )
                    .with_prefix("input".to_owned())
                    .with("commodity", commodity)
                    .with("service_id", service_id)
                    .with("metadata", metadata);
                //.with("method.proof", proof);
            },
            UpdateMethod::Withdraw {
                amount,
                token,
                receiving_address,
            } => {
                random_oracle = random_oracle
                    .with("transaction_name", &"withdraw")
                    .with_prefix("input".to_owned())
                    .with("amount", amount)
                    .with("token", token)
                    .with("receiving_address", &receiving_address.0);
            },
            UpdateMethod::Stake {
                amount,
                node_public_key,
                node_network_key,
                node_domain,
                worker_public_key,
                worker_domain,
                worker_mempool_address,
            } => {
                random_oracle = random_oracle
                    .with("transaction_name", &"stake")
                    .with_prefix("input".to_owned())
                    .with("amount", amount)
                    .with("node_public_key", &node_public_key.0)
                    .with(
                        "node_network_key",
                        &node_network_key.map_or([0u8; 32], |key| key.0),
                    )
                    .with("node_domain", node_domain)
                    .with(
                        "worker_public_key",
                        &worker_public_key.map_or([0u8; 32], |key| key.0),
                    )
                    .with("worker_domain", worker_domain)
                    .with("worker_mempool_address", worker_mempool_address);
            },
            UpdateMethod::StakeLock { node, locked_for } => {
                random_oracle = random_oracle
                    .with("transaction_name", &"stake_lock")
                    .with_prefix("input".to_owned())
                    .with("node", &node.0)
                    .with("locked_for", locked_for);
            },
            UpdateMethod::Unstake { amount, node } => {
                random_oracle = random_oracle
                    .with("transaction_name", &"unstake")
                    .with_prefix("input".to_owned())
                    .with("node", &node.0)
                    .with("amount", amount);
            },
            UpdateMethod::WithdrawUnstaked { node, recipient } => {
                random_oracle = random_oracle
                    .with("transaction_name", &"withdraw_unstaked")
                    .with_prefix("input".to_owned())
                    .with("node", &node.0)
                    .with("recipient", &recipient.map_or([0u8; 20], |key| key.0));
            },
            UpdateMethod::ChangeEpoch { epoch } => {
                random_oracle = random_oracle
                    .with("transaction_name", &"change_epoch")
                    .with_prefix("input".to_owned())
                    .with("epoch", epoch);
            },
            UpdateMethod::AddService {
                service,
                service_id,
            } => {
                random_oracle = random_oracle
                    .with("transaction_name", &"add_service")
                    .with_prefix("input".to_owned())
                    .with("service_id", service_id)
                    .with("service", service);
            },
            UpdateMethod::RemoveService { service_id } => {
                random_oracle = random_oracle
                    .with("transaction_name", &"remove_service")
                    .with_prefix("input".to_owned())
                    .with("service_id", service_id);
            },
            UpdateMethod::Slash {
                service_id,
                node,
                proof_of_misbehavior: _,
            } => {
                random_oracle = random_oracle
                    .with("transaction_name", &"slash")
                    .with_prefix("input".to_owned())
                    .with("service_id", service_id)
                .with("node", &node.0)
                // .with("proof_of_misbehavior", proof_of_misbehavior)
                ;
            },
            UpdateMethod::SubmitReputationMeasurements { measurements } => {
                random_oracle =
                    random_oracle.with("transaction_name", &"submit_reputation_measurements");
                for (key, value) in measurements {
                    random_oracle = random_oracle
                        .with_prefix(key.to_base64())
                        .with("latency", &value.latency.map_or(0, |l| l.as_nanos()))
                        .with("interactions", &value.interactions)
                        .with("inbound_bandwidth", &value.inbound_bandwidth)
                        .with("outbound_bandwidth", &value.outbound_bandwidth)
                        .with("bytes_received", &value.bytes_received)
                        .with("bytes_sent", &value.bytes_sent)
                        .with("hops", &value.hops);
                }
            },
        }

        derive_key(random_oracle.get_domain(), &random_oracle.compile())
    }
}

/// Placeholder
/// This is the proof presented to the slashing function that proves a node misbehaved and should be
/// slashed
#[derive(Clone, Copy, Debug, Serialize, Deserialize, Hash)]
pub struct ProofOfMisbehavior {}

/// Placeholder
/// This is the proof used to operate our PoC bridges
#[derive(Clone, Debug, Serialize, Deserialize, Hash)]
pub struct ProofOfConsensus {}

/// Contains the peer measurements that node A has about node B, that
/// will be taken into account when computing B's reputation score.
#[derive(Clone, Debug, Eq, PartialEq, Default, Hash, Serialize, Deserialize)]
pub struct ReputationMeasurements {
    pub latency: Option<Duration>,
    pub interactions: Option<i64>,
    pub inbound_bandwidth: Option<u128>,
    pub outbound_bandwidth: Option<u128>,
    pub bytes_received: Option<u128>,
    pub bytes_sent: Option<u128>,
    pub hops: Option<u8>,
}

#[derive(Clone, Debug, Hash, Serialize, Deserialize)]
pub struct ReportedReputationMeasurements {
    pub reporting_node: NodePublicKey,
    pub measurements: ReputationMeasurements,
}

impl RandomOracleInput for Tokens {
    const TYPE: &'static str = "Tokens";

    fn to_random_oracle_input(&self) -> Vec<u8> {
        match self {
            Tokens::USDC => b"USDC".to_vec(),
            Tokens::FLK => b"FLK".to_vec(),
        }
    }
}

impl RandomOracleInput for CommodityTypes {
    const TYPE: &'static str = "commodity_types";

    fn to_random_oracle_input(&self) -> Vec<u8> {
        match self {
            CommodityTypes::Bandwidth => b"Bandwidth".to_vec(),
            CommodityTypes::Compute => b"Compute".to_vec(),
            CommodityTypes::Gpu => b"Gpu".to_vec(),
        }
    }
}

impl RandomOracleInput for Service {
    const TYPE: &'static str = "service";

    fn to_random_oracle_input(&self) -> Vec<u8> {
        self.commodity_type.to_random_oracle_input()
        // todo: check if implementation needs to change when slashing is implemented
    }
}
