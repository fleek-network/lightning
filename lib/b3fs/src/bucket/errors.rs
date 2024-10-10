use thiserror::Error;
use triomphe::UniqueArc;

use crate::hasher::collector::InvalidHashSize;

#[derive(Error, Debug)]
pub enum ReadError {
    #[error("Error while reading from disk. {0}")]
    ReadingFile(#[from] std::io::Error),
    #[error("Error getting mut ref to file")]
    RefFile,
    #[error("Error while converting hash tree")]
    HashTreeConversion,
}

#[derive(Error, Debug)]
pub enum FeedProofError {
    #[error("Putter was running without incremental verification.")]
    UnexpectedCall,
    #[error("Proof is not matching the current root.")]
    InvalidProof,
}

#[derive(Error, Debug)]
pub enum WriteError {
    #[error("The provided content is not matching the hash.")]
    InvalidContent,
    #[error("The provided content could not be decompressed.")]
    DecompressionFailure,
    #[error("Error while writing to disk. {0}")]
    IOError(#[from] std::io::Error),
    #[error("Error while hashing the block. Not found hash for block.")]
    BlockHashNotFound,
    #[error("Invalid block hash detected against incremental verifier")]
    InvalidBlockHash,
    #[error("Error while getting current block file. Not found")]
    BlockFileNotFound,
}

#[derive(Error, Debug)]
pub enum InsertError {
    #[error("The provided entry does not meet the expected hash.")]
    InvalidContent,
    #[error("The provided entry violates the entry name ordering.")]
    OrderingError,
}

#[derive(Error, Debug)]
pub enum CommitError {
    #[error("The putter expected more data to come to a finalized state.")]
    PartialContent,
    #[error("The final CID does not match the CID which was expected.")]
    InvalidCID,
    #[error("Writing to disk failed.")]
    WriteFailed,
    #[error("Error while writing to disk. {0}")]
    IOError(#[from] std::io::Error),
    #[error("Invalid Hash or internal hashes content. {0}")]
    InvalidProof(#[from] InvalidHashSize),
    #[error("Error while hashing the block. Not found hash for block.")]
    BlockHashNotFound,
    #[error("Invalid block hash detected against incremental verifier")]
    InvalidBlockHash,
}
