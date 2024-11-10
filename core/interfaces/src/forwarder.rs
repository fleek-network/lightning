use affair::Socket;
use fdi::BuildGraph;
use lightning_types::{ForwarderError, TransactionRequest};

use crate::components::NodeComponents;

/// A socket that gives services and other sub-systems the required functionality to
/// submit messages/transactions to the consensus.
///
/// # Safety
///
/// This socket is safe to freely pass around, sending transactions through this socket
/// does not guarantee their execution on the application layer. You can think about
/// this as if the current node was only an external client to the network.
pub type MempoolSocket = Socket<TransactionRequest, Result<(), ForwarderError>>;

#[interfaces_proc::blank]
pub trait ForwarderInterface<C: NodeComponents>: BuildGraph + Sized + Send + 'static {
    /// Get the socket for forwarding new transaction requests to the mempool.
    #[socket]
    fn mempool_socket(&self) -> MempoolSocket;
}
