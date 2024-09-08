use thiserror::Error;

#[derive(Error, Debug, PartialEq, Eq)]
pub enum FleekCryptoError {
    #[error("invalid public key: {0}")]
    InvalidPublicKey(String),
    #[error("invalid signature: {0}")]
    InvalidSignature(String),
    #[error("failed to aggregate signatures: {0}")]
    AggregateSignaturesFailure(String),
    #[error("invalid aggregate signature: {0}")]
    InvalidAggregateSignature(String),
}
