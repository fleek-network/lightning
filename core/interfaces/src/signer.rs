use affair::Socket;
use fdi::BuildGraph;
use lightning_types::{
    ExecuteTransactionError,
    ExecuteTransactionRequest,
    ExecuteTransactionResponse,
};
use thiserror::Error;

use crate::components::NodeComponents;

/// A socket that is responsible to submit a transaction to the consensus from our node,
/// implementation of this socket needs to assure the consistency and increment of the
/// nonce (which we also refer to as the counter).
pub type SignerSubmitTxSocket =
    Socket<ExecuteTransactionRequest, Result<ExecuteTransactionResponse, SignerError>>;

/// The signature provider is responsible for signing messages using the private key of
/// the node.
#[interfaces_proc::blank]
pub trait SignerInterface<C: NodeComponents>: BuildGraph + Sized + Send + Sync {
    /// Returns a socket that can be used to submit transactions to the mempool, these
    /// transactions are signed by the node and a proper nonce is assigned by the
    /// implementation.
    #[socket]
    fn get_socket(&self) -> SignerSubmitTxSocket;
}

#[derive(Debug, Error, Eq, PartialEq)]
pub enum SignerError {
    /// The transaction execution failed.
    #[error(transparent)]
    ExecuteTransaction(#[from] ExecuteTransactionError),

    /// The signer is not ready.
    #[error("Signer not ready")]
    NotReady,

    /// The signer is being shutdown.
    #[error("Signer is shutting down")]
    ShuttingDown,

    /// Other generic error.
    #[error("Other: {:?}", .0)]
    Other(String),
}

impl From<SignerError> for ExecuteTransactionError {
    fn from(err: SignerError) -> Self {
        match err {
            SignerError::ExecuteTransaction(e) => e,
            SignerError::Other(e) => ExecuteTransactionError::Other(e),
            SignerError::NotReady => ExecuteTransactionError::SignerNotReady,
            SignerError::ShuttingDown => ExecuteTransactionError::NotifierShuttingDown,
        }
    }
}

impl From<anyhow::Error> for SignerError {
    fn from(error: anyhow::Error) -> Self {
        Self::Other(error.to_string())
    }
}

impl From<String> for SignerError {
    fn from(error: String) -> Self {
        Self::Other(error)
    }
}
