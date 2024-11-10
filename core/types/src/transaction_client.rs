use std::time::Duration;

use affair::RunError;
use thiserror::Error;

use crate::{ExecutionError, ForwarderError, TransactionReceipt, TransactionRequest, UpdateMethod};

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
            _ => unreachable!("invalid receipt in response: {:?}", self),
        }
    }

    pub fn as_none(&self) {
        match self {
            Self::None => (),
            _ => unreachable!("invalid receipt in response: {:?}", self),
        }
    }
}

#[derive(Debug, Clone, Default, Eq, PartialEq)]
pub enum ExecuteTransactionWait {
    #[default]
    None,
    Receipt,
}

#[derive(Debug, Clone, Default, Eq, PartialEq)]
pub enum ExecuteTransactionRetry {
    #[default]
    Default,
    Never,
}

impl ExecuteTransactionRetry {
    pub fn get_max_retries(&self, default: MaxRetries) -> MaxRetries {
        match self {
            Self::Default => default,
            Self::Never => 0,
        }
    }

    pub fn should_retry_on_forwarder_error(
        &self,
        error: &ForwarderError,
    ) -> Option<(MaxRetries, Duration)> {
        match self {
            Self::Default => match error {
                // Retry on no active connections error.
                // This can happen sometimes if the forwarder is not ready yet, so retry a bunch of
                // times with a moderate delay, since it's likely to happen on startup.
                ForwarderError::NoActiveConnections => Some((50, Duration::from_millis(500))),
                // Retry on failed to send to any connection error.
                // This can happen sometimes if the forwarder is not ready yet, so retry a bunch of
                // times with a moderate delay, since it's likely to happen on startup.
                ForwarderError::FailedToSendToAnyConnection => {
                    Some((50, Duration::from_millis(500)))
                },
                // Otherwise, don't retry.
                ForwarderError::FailedToSerializeTransaction(_) => None,
            },
            Self::Never => None,
        }
    }

    pub fn should_retry_on_forwarder_run_error(
        &self,
        error: &RunError<TransactionRequest>,
    ) -> Option<(MaxRetries, Duration)> {
        match self {
            Self::Default => match error {
                // Retry on enqueue error.
                // This can happen sometimes if the component is not ready yet, so retry a bunch of
                // times with a moderate delay, since it's likely to happen on startup.
                RunError::FailedToEnqueueReq(_) => Some((50, Duration::from_millis(500))),
                // Retry on get response error.
                // This can happen sometimes if the component is not ready yet, so retry a bunch of
                // times with a moderate delay, since it's likely to happen on startup.
                RunError::FailedToGetResponse => Some((50, Duration::from_millis(500))),
            },
            Self::Never => None,
        }
    }

    pub fn should_retry_on_revert(&self, error: &ExecutionError) -> Option<(MaxRetries, Duration)> {
        match self {
            Self::Default => match error {
                // Retry on invalid nonce error.
                // This can happen sometimes, so retry a few times with a short delay.
                ExecutionError::InvalidNonce => Some((3, Duration::from_millis(100))),
                // Otherwise, don't retry.
                _ => None,
            },
            Self::Never => None,
        }
    }

    pub fn should_retry_on_timeout(&self) -> bool {
        match self {
            Self::Default => true,
            Self::Never => false,
        }
    }
}
pub type RetryOnTimeout = bool;

pub type Attempts = u8;

#[derive(Debug, Error, Eq, PartialEq)]
pub enum ExecuteTransactionError {
    // The transaction was submitted but reverted during execution for the reason in the receipt.
    #[error("Transaction was reverted: {:?}", .0.0.hash())]
    Reverted((TransactionRequest, TransactionReceipt, Attempts)),

    /// The transaction execution timed out.
    #[error("Transaction timeout: {:?}", .0)]
    Timeout((UpdateMethod, Option<TransactionRequest>, Attempts)),

    /// The transaction was not submitted to the signer.
    #[error("Failed to submit transaction to signer: {:?}", .0)]
    FailedToSubmitRequestToSigner(ExecuteTransactionRequest),

    /// The transaction failed to be forwarded to the mempool.
    #[error("Failed to forward transaction to mempool (tx: {:?}): {:?}", .0.0.hash(), .0.1)]
    ForwarderError((TransactionRequest, ForwarderError)),

    /// The transaction failed to be forwarded to the mempool.
    #[error("Failed to forward transaction to mempool (tx: {:?}): {:?}", .0.0.hash(), .0.1)]
    ForwarderRunError((TransactionRequest, RunError<TransactionRequest>)),

    /// Failed to get response from signer.
    #[error("Failed to get response from signer")]
    FailedToGetResponseFromSigner,

    /// Failed to increment nonce for retry.
    #[error("Failed to increment nonce for retry (tx: {:?}): {:?}", .0.0.hash(), .0.1)]
    FailedToIncrementNonceForRetry((TransactionRequest, String)),

    /// The transaction was executed but the receipt is not available.
    #[error("Transaction executed but receipt not available: {:?}", .0)]
    TransactionExecutedButReceiptNotAvailable(TransactionRequest),

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
