use serde::{Deserialize, Serialize};

use crate::digest::ToDigest;
use crate::identity::{BlsPublicKey, Ed25519PublicKey, PeerId, Signature};

/// Unix time stamp in second.
pub type UnixTs = u64;

/// A block of transactions, which is a list of update requests each signed by a user,
/// the block is the atomic view into the network, meaning that queries do not view
/// the intermediary state within a block, but only have the view to the latest executed
/// block.
#[derive(Debug)]
pub struct Block {
    pub timestamp: UnixTs,
    pub transactions: Vec<UpdateRequest>,
}

/// An update transaction, sent from users to the consensus to migrate the application
/// from one state to the next state.
///
#[derive(Debug, Hash)]
pub struct UpdateRequest {
    /// The sender of the transaction.
    pub sender: PeerId,
    /// The signature by the user signing this payload.
    pub signature: Signature,
    /// The payload of an update request, which contains a counter (nonce), and
    /// the transition function itself.
    pub payload: UpdatePayload,
}

/// A query request, which is still signed and submitted by a user.
#[derive(Debug, Hash)]
pub struct QueryRequest {
    /// The sender of this query request.
    pub sender: PeerId,
    /// The signature on this payload.
    pub signature: Signature,
    /// The query function.
    pub query: QueryMethod,
}

/// The payload data of an update request.
#[derive(Debug, Hash)]
pub struct UpdatePayload {
    /// The counter or nonce of this request.
    pub counter: u64,
    /// The transition function (and parameters) for this update request.
    pub method: UpdateMethod,
}

/// All of the update functions in our logic, along their parameters.
#[derive(Debug, Hash)]
pub enum UpdateMethod {
    // TODO
}

/// All of the query functions in our logic, along their parameters.
#[derive(Debug, Hash)]
pub enum QueryMethod {
    // TODO
}

/// The serialized response from executing a query.
pub type QueryResponse = Vec<u8>;

#[derive(Debug, Hash, PartialEq, PartialOrd, Ord, Eq, Serialize, Deserialize)]
pub struct NodeInfo {
    /// The BLS public key of the node which is used for our BFT DAG consensus
    /// multi signatures.
    pub public_key: BlsPublicKey,
    /// Public key that is used for fast communication signatures for this node.
    pub network_public_key: Ed25519PublicKey,
    /// The time stamp that this node staked at,
    pub staked_since: u64,
    /// The amount of stake by the node.
    pub stake: u128,
    /// Services that this node is running.
    pub services: Vec<ServiceId>,
    /// Different addresses that points to the same node.
    pub addresses: Vec<InternetAddress>,
}

#[derive(Debug, Hash, PartialEq, PartialOrd, Ord, Eq, Serialize, Deserialize)]
pub struct ServiceId(pub [u8; 32]);

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
#[derive(Debug, Hash, PartialEq, PartialOrd, Ord, Eq, Serialize, Deserialize)]
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
