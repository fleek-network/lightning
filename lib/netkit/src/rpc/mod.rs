pub mod client;
pub mod server;

use thiserror::Error;

#[derive(Debug, Error)]
#[error("UDP transport error: {0}")]
pub struct TransportError(String);
