use std::error::Error;

use ethers::utils::rlp;
use jsonrpsee::types::error::INTERNAL_ERROR_CODE;
use jsonrpsee::types::ErrorObject;
use ruint::ParseError;

#[derive(Debug)]
pub struct SocketErrorWrapper(String);

impl std::ops::Deref for SocketErrorWrapper {
    type Target = String;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl Error for SocketErrorWrapper {}

impl std::fmt::Display for SocketErrorWrapper {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

#[derive(Debug, thiserror::Error)]
pub enum RPCError {
    #[error("Failed to decode RLP Data {}", .0)]
    DecoderError(#[from] rlp::DecoderError),

    #[error("Failed to verify signature {}", .0)]
    SignatureError(#[from] ethers::types::SignatureError),

    #[error("Recieved an error from the socket {}", *.0)]
    SocketError(#[from] SocketErrorWrapper),

    #[error("Unimplemented")]
    Unimplemented,

    #[error("Failed to parse uint {}", .0)]
    ParseError(#[from] ParseError),

    #[error("RPCError {}", .0)]
    Custom(String),

    #[error("Bad Epoch")]
    BadEpoch,

    #[error("Not an archive node")]
    NotArchiveNode,

    #[error("Error: ")]
    Anyhow(#[from] anyhow::Error),
}

impl RPCError {
    pub fn unimplemented() -> Self {
        Self::Unimplemented
    }

    pub fn socket(s: String) -> Self {
        Self::SocketError(SocketErrorWrapper(s))
    }

    pub fn custom(s: String) -> Self {
        Self::Custom(s)
    }
}

impl<T> From<affair::RunError<T>> for SocketErrorWrapper {
    fn from(e: affair::RunError<T>) -> Self {
        Self(e.to_string())
    }
}

impl<T> From<affair::RunError<T>> for RPCError {
    fn from(e: affair::RunError<T>) -> Self {
        Self::from(SocketErrorWrapper::from(e))
    }
}

impl From<RPCError> for ErrorObject<'static> {
    fn from(e: RPCError) -> Self {
        match e {
            RPCError::SignatureError(e) => internal_err(e),
            RPCError::DecoderError(e) => internal_err(e),
            RPCError::SocketError(e) => internal_err(e),
            RPCError::Unimplemented => internal_err_from_string("Unimplemented".to_string()),
            RPCError::Custom(s) => internal_err_from_string(s),
            RPCError::ParseError(e) => internal_err(e),
            RPCError::BadEpoch => internal_err_from_string("Bad Epoch".to_string()),
            RPCError::Anyhow(e) => internal_err_from_string(e.to_string()),
            RPCError::NotArchiveNode => internal_err_from_string(e.to_string()),
        }
    }
}

fn internal_err<E: Error>(e: E) -> ErrorObject<'static> {
    jsonrpsee::types::ErrorObject::owned::<()>(INTERNAL_ERROR_CODE, e.to_string(), None)
}

fn internal_err_from_string(s: String) -> ErrorObject<'static> {
    jsonrpsee::types::ErrorObject::owned::<()>(INTERNAL_ERROR_CODE, s, None)
}
