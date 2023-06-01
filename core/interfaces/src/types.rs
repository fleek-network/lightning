use fleek_crypto::{
    AccountOwnerPublicKey, ClientPublicKey, NodeNetworkingPublicKey, NodePublicKey,
    TransactionSender, TransactionSignature,
};
use serde::{Deserialize, Serialize};

use crate::{common::ToDigest, pod::DeliveryAcknowledgment};

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
    pub timestamp: UnixTs,
    pub transactions: Vec<UpdateRequest>,
}

#[derive(Serialize, Deserialize, Hash, Debug, Clone)]
pub enum Tokens {
    USDC,
    FLK,
}

/// Placeholder
/// Information about the services
#[derive(Clone, Copy, Debug, Serialize, Deserialize, Hash)]
pub struct Service {
    pub commodity_price: u128,
    /// TODO: List of circuits to prove a node should be slashed
    pub slashing: (),
}

/// An update transaction, sent from users to the consensus to migrate the application
/// from one state to the next state.
#[derive(Debug, Hash, Clone)]
pub struct UpdateRequest {
    /// The sender of the transaction.
    pub sender: TransactionSender,
    /// The signature by the user signing this payload.
    pub signature: TransactionSignature,
    /// The payload of an update request, which contains a counter (nonce), and
    /// the transition function itself.
    pub payload: UpdatePayload,
}

/// A query request, which is still signed and submitted by a user.
#[derive(Debug, Hash, Clone)]
pub struct QueryRequest {
    /// The sender of this query request.
    pub sender: TransactionSender,
    /// The query function.
    pub query: QueryMethod,
}

/// The payload data of an update request.
#[derive(Debug, Hash, Clone)]
pub struct UpdatePayload {
    /// The counter or nonce of this request.
    pub counter: u64,
    /// The transition function (and parameters) for this update request.
    pub method: UpdateMethod,
}

/// All of the update functions in our logic, along their parameters.
#[derive(Debug, Hash, Clone)]
pub enum UpdateMethod {
    /// The main function of the application layer. After aggregating ProofOfAcknowledgements a
    /// node will submit this     transaction to get paid.
    /// Revisit the naming of this transaction.
    SubmitDeliveryAcknowledgmentAggregation {
        /// How much of the commodity was served
        commodity: u128,
        /// The service id of the service this was provided through(CDN, compute, ect.)
        service_id: u64,
        /// The PoD of delivery in bytes
        proofs: Vec<DeliveryAcknowledgment>,
        /// Optional metadata to provide information additional information about this batch
        metadata: Option<Vec<u8>>,
    },
    /// Withdraw tokens from the network back to the L2
    Withdraw {
        /// The amount to withdrawl
        amount: u128,
        /// Which token to withdrawl
        token: Tokens,
        /// The address to recieve these tokens on the L2
        receiving_address: AccountOwnerPublicKey,
    },
    /// Submit of PoC from the bridge on the L2 to get the tokens in network
    Deposit {
        /// The proof of the bridge recieved from the L2,
        proof: ProofOfConsensus,
        /// Which token was bridged
        token: Tokens,
        /// Amount bridged
        amount: u128,
    },
    /// Stake FLK in network
    Stake {
        /// The proof of the bridge recieved from the L2,
        proof: ProofOfConsensus,
        /// Amount bridged
        amount: u128,
        /// Node Public Key
        node: NodePublicKey,
    },
    /// Unstake FLK, the tokens will be locked for a set amount of
    /// time(ProtocolParameter::LockTime) before they can be withdrawn
    Unstake { amount: u128 },
    /// Sent by committee member to signal he is ready to change epoch
    ChangeEpoch,
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
}

/// All of the query functions in our logic, along their parameters.
#[derive(Debug, Hash, Clone)]
pub enum QueryMethod {
    /// Get the balance of unlocked FLK a public key has
    FLK { public_key: AccountOwnerPublicKey },
    /// Get the balance of locked FLK a public key has
    Locked { public_key: AccountOwnerPublicKey },
    /// Get the amount of prepaid bandwidth a public key has
    Bandwidth { public_key: ClientPublicKey },
    /// Get the amount of stake a node has
    Staked { node: NodePublicKey },
    /// Get the amount of bandwidth served in an epoch
    Served {
        /// the epoch
        epoch: Epoch,
        /// The node public Key
        node: NodePublicKey,
    },
    /// Get the total served for all nodes in an epoch
    TotalServed { epoch: Epoch },
    /// Get the amount in the reward pool for an epoch
    RewardPool { epoch: Epoch },
    /// Get the current epoch information
    CurrentEpochInfo,
}

/// The serialized response from executing a query.
pub type QueryResponse = Vec<u8>;

/// The account info stored per account on the blockchain
#[derive(Debug, Hash, PartialEq, PartialOrd, Ord, Eq, Serialize, Deserialize)]
pub struct AccountInfo {
    pub flk_balance: u128,
    pub bandwidth_balance: u128,
    pub nonce: u128,
    pub staking: Staking,
}

/// Struct that stores
#[derive(Debug, Hash, PartialEq, PartialOrd, Ord, Eq, Serialize, Deserialize)]
pub struct Staking {
    pub staked: u128,
    pub locked: u128,
    pub locked_until: u64,
}

#[derive(Debug, Hash, PartialEq, PartialOrd, Ord, Eq, Serialize, Deserialize)]
pub struct NodeInfo {
    /// The BLS public key of the node which is used for our BFT DAG consensus
    /// multi signatures.
    pub public_key: NodePublicKey,
    /// Public key that is used for fast communication signatures for this node.
    pub network_public_key: NodeNetworkingPublicKey,
    /// The time stamp that this node staked at,
    pub staked_since: u64,
    /// The amount of stake by the node.
    pub stake: u128,
    /// Services that this node is running.
    pub services: Vec<ServiceId>,
    /// Different addresses that points to the same node.
    pub addresses: Vec<InternetAddress>,
}

/// Metadata, state stored in the blockchain that applies to the current block
#[derive(Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Metadata {
    Epoch,
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
    ProtocolPercentage = 5,
    /// The maximum targed inflation rate in a year
    MaxInflation = 6,
    /// The minimum targeted inflation rate in a year
    MinInflation = 7,
    /// The amount of FLK minted per GB they consume.
    ConsumerRebate = 8,
}

/// Error type for transaction execution on the application layer
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
        todo!()
    }
}

impl ToDigest for QueryMethod {
    /// Computes the hash of this query request and returns a 32-byte hash.
    fn to_digest(&self) -> [u8; 32] {
        todo!()
    }
}

/// Placeholder
/// This is the proof presented to the slashing function that proves a node misbehaved and should be
/// slashed
#[derive(Clone, Copy, Debug, Serialize, Deserialize, Hash)]
pub struct ProofOfMisbehavior {}

/// Placeholder
/// This is the proof used to operate our PoC bridges
#[derive(Clone, Copy, Debug, Serialize, Deserialize, Hash)]
pub struct ProofOfConsensus {}
