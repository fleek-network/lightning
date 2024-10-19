use thiserror::Error;

#[derive(Error, Debug)]
pub enum NodeError {
    #[error("timeout waiting for node to be ready")]
    WaitForReadyTimeout(#[from] tokio::time::error::Elapsed),

    #[error("Internal: {0}")]
    Internal(String),
}
