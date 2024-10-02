use lightning_interfaces::types::{TransactionReceipt, TransactionRequest};
use thiserror::Error;
use tokio::sync::mpsc;

/// The transaction client error enum encapsulates errors that can occur when executing transactions
/// with the [`TransactionClient`].
#[derive(Debug, Error)]
pub enum TransactionClientError {
    // The transaction was submitted but reverted during execution for the reason in the receipt.
    #[error("transaction was reverted: {:?}", .0.0.hash())]
    Reverted((TransactionRequest, TransactionReceipt)),

    /// The transaction exceeded timeout that encompasses the whole execution process including
    /// waiting and retries.
    #[error("transaction timeout: {:?}", .0.hash())]
    Timeout(TransactionRequest),

    /// The transaction was submitted but the receipt was not received before the timeout.
    #[error("transaction timeout waiting for receipt: {:?}", .0.hash())]
    TimeoutWaitingForReceipt(TransactionRequest),

    /// The transaction failed to send to mempool.
    #[error("transaction failed to send to mempool: {0:?}")]
    MempoolSendFailed(mpsc::error::SendError<TransactionRequest>),

    /// An internal or unknown error occurred.
    #[error("internal error: {0}")]
    Internal(String),
}
