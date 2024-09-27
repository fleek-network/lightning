use affair::Socket;
use fdi::BuildGraph;

use crate::components::NodeComponents;
use crate::types::UpdateMethod;

/// A socket that is responsible to submit a transaction to the consensus from our node,
/// implementation of this socket needs to assure the consistency and increment of the
/// nonce (which we also refer to as the counter).
pub type SubmitTxSocket = Socket<UpdateMethod, u64>;

/// The signature provider is responsible for signing messages using the private key of
/// the node.
#[interfaces_proc::blank]
pub trait SignerInterface<C: NodeComponents>: BuildGraph + Sized + Send + Sync {
    /// Returns a socket that can be used to submit transactions to the mempool, these
    /// transactions are signed by the node and a proper nonce is assigned by the
    /// implementation.
    #[socket]
    fn get_socket(&self) -> SubmitTxSocket;
}
