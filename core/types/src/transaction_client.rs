use std::time::Duration;

use thiserror::Error;

use crate::{ExecutionError, TransactionReceipt, TransactionRequest, UpdateMethod};

type MaxRetries = u8;

type Timeout = Duration;

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct ExecuteTransactionRequest {
    pub method: UpdateMethod,
    pub options: Option<ExecuteTransactionOptions>,
}

#[derive(Debug, Clone, Default, Eq, PartialEq)]
pub struct ExecuteTransactionOptions {
    pub retry: ExecuteTransactionRetry,
    pub wait: ExecuteTransactionWait,
    pub timeout: Option<Timeout>,
}

#[derive(Debug, Clone, Default, Eq, PartialEq)]
pub enum ExecuteTransactionResponse {
    #[default]
    None,
    Receipt((TransactionRequest, TransactionReceipt)),
}

impl ExecuteTransactionResponse {
    pub fn as_receipt(&self) -> (TransactionRequest, TransactionReceipt) {
        match self {
            Self::Receipt(v) => v.clone(),
            _ => unreachable!("invalid receipt in response"),
        }
    }

    pub fn as_none(&self) {
        match self {
            Self::None => (),
            _ => unreachable!("invalid receipt in response"),
        }
    }
}

#[derive(Debug, Clone, Default, Eq, PartialEq)]
pub enum ExecuteTransactionWait {
    #[default]
    None,
    Receipt(Option<Timeout>),
}

#[derive(Debug, Clone, Default, Eq, PartialEq)]
pub enum ExecuteTransactionRetry {
    #[default]
    Default,
    Never,
    Always(Option<MaxRetries>),
    AlwaysExcept((Option<MaxRetries>, Option<Vec<ExecutionError>>)),
    OnlyWith((Option<MaxRetries>, Option<Vec<ExecutionError>>)),
}

#[derive(Debug, Error, Eq, PartialEq)]
pub enum ExecuteTransactionError {
    // The transaction was submitted but reverted during execution for the reason in the receipt.
    #[error("Transaction was reverted: {:?}", .0.0.hash())]
    Reverted((TransactionRequest, TransactionReceipt)),

    /// The transaction execution timed out.
    #[error("Transaction timeout: {:?}", .0)]
    Timeout((UpdateMethod, Option<TransactionRequest>)),

    /// The transaction was not submitted to the signer.
    #[error("Failed to submit transaction to signer: {:?}", .0)]
    FailedToSubmitRequestToSigner(ExecuteTransactionRequest),

    /// The transaction failed to be submitted to the mempool.
    #[error("Failed to submit transaction to mempool: {:?}: {:?}", .0.0.hash(), .0.1)]
    FailedToSubmitTransactionToMempool((TransactionRequest, String)),

    /// Failed to get response from signer.
    #[error("Failed to get response from signer")]
    FailedToGetResponseFromSigner,

    /// The signer is not ready.
    #[error("Signer not ready")]
    SignerNotReady,

    /// The notifier subscription has been closed, indicating that it's shutting down.
    #[error("Notifier is shutting down")]
    NotifierShuttingDown,

    /// Other generic error.
    #[error("Other: {:?}", .0)]
    Other(String),
}

impl From<affair::RunError<ExecuteTransactionRequest>> for ExecuteTransactionError {
    fn from(err: affair::RunError<ExecuteTransactionRequest>) -> Self {
        match err {
            affair::RunError::FailedToEnqueueReq(req) => {
                ExecuteTransactionError::FailedToSubmitRequestToSigner(req)
            },
            affair::RunError::FailedToGetResponse => {
                ExecuteTransactionError::FailedToGetResponseFromSigner
            },
        }
    }
}

impl From<tokio::task::JoinError> for ExecuteTransactionError {
    fn from(error: tokio::task::JoinError) -> Self {
        Self::Other(error.to_string())
    }
}

impl From<anyhow::Error> for ExecuteTransactionError {
    fn from(error: anyhow::Error) -> Self {
        Self::Other(error.to_string())
    }
}

impl From<String> for ExecuteTransactionError {
    fn from(error: String) -> Self {
        Self::Other(error)
    }
}
