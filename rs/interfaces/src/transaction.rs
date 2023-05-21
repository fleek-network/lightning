use crate::digest::ToDigest;
use crate::identity::{PeerId, Signature};

/// A block of transactions, which is a list of update requests each signed by a user,
/// the block is the atomic view into the network, meaning that queries do not view
/// the intermediary state within a block, but only have the view to the latest executed
/// block.
#[derive(Debug)]
pub struct Block(pub Vec<UpdateRequest>);

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
