use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error("Internal blockstore error: {0}")]
    Blockstore(String),
    #[error("Parsing car file failed: {0}")]
    CarReader(String),
}
