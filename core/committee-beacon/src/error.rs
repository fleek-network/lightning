use lightning_interfaces::types::Value;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum CommitteeBeaconError {
    #[error("committee beacon unknown phase type: {0:?}")]
    UnknownPhaseType(Option<Value>),

    #[error("own node not found")]
    OwnNodeNotFound,

    #[error("failed to submit transaction: {0}")]
    SubmitTransaction(#[from] SocketErrorWrapper),

    #[error("timeout waiting for transaction receipt: {0:?}")]
    TimeoutWaitingForTransactionReceipt([u8; 32]),

    #[error("timeout waiting for transaction to be executed: {0:?}")]
    TimeoutWaitingForTransactionExecution([u8; 32]),

    #[error("committee beacon error: {0}")]
    Unknown(String),
}

impl From<anyhow::Error> for CommitteeBeaconError {
    fn from(error: anyhow::Error) -> Self {
        CommitteeBeaconError::Unknown(error.to_string())
    }
}

impl From<String> for CommitteeBeaconError {
    fn from(error: String) -> Self {
        CommitteeBeaconError::Unknown(error)
    }
}

impl<T> From<affair::RunError<T>> for SocketErrorWrapper {
    fn from(e: affair::RunError<T>) -> Self {
        Self(e.to_string())
    }
}

impl<T> From<affair::RunError<T>> for CommitteeBeaconError {
    fn from(e: affair::RunError<T>) -> Self {
        Self::from(SocketErrorWrapper::from(e))
    }
}

#[derive(Debug)]
pub struct SocketErrorWrapper(String);

impl std::ops::Deref for SocketErrorWrapper {
    type Target = String;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl std::error::Error for SocketErrorWrapper {}

impl std::fmt::Display for SocketErrorWrapper {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}
