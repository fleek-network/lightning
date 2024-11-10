use thiserror::Error;

#[derive(Error, Debug, Eq, PartialEq)]
pub enum ForwarderError {
    #[error("no active connections to forward to")]
    NoActiveConnections,

    #[error("failed to send transaction to any connection")]
    FailedToSendToAnyConnection,

    #[error("failed to serialize transaction: {0}")]
    FailedToSerializeTransaction(String),
}
