use thiserror::Error;
use triomphe::UniqueArc;

use crate::hasher::collector::InvalidHashSize;
use crate::{hasher, stream};

#[derive(Error, Debug)]
pub enum ReadError {
    #[error("Error while reading from disk. {0}")]
    ReadingFile(#[from] std::io::Error),
    #[error("Error getting mut ref to file")]
    RefFile,
    #[error("Error while converting hash tree")]
    HashTreeConversion,
    #[error("Invalid hashtree {0}")]
    InvalidHashtree(#[from] crate::collections::error::CollectionTryFromError),
    #[error("Error while deserializing PHF table. {0}")]
    Deserialization(String),
    #[error("Invalid entry at offset {2}. Required name {0:?}, found {1:?}")]
    InvalidEntryAtOffset(Vec<u8>, Vec<u8>, u32),
    #[error("Invalid bloom filter")]
    InvalidBloomFilter,
}

#[derive(Error, Debug)]
pub enum FeedProofError {
    #[error("Putter was running without incremental verification.")]
    UnexpectedCall,
    #[error("Proof is not matching the current root.")]
    InvalidProof,
    #[error("Error while feeding proof. {0}")]
    VerificationError(#[from] stream::verifier::VerificationError),
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
    #[error("Error while writing to disk. {0}")]
    WritingError(#[from] WriteError),
    #[error("Error writing to disk. {0}")]
    IO(#[from] std::io::Error),
    #[error("Invalid Hash or internal hashes content doing incremental validation")]
    IncrementalVerification,
    #[error("Error trying to insert entry in phf generator. {0}")]
    InvalidEntry(#[from] hasher::dir_hasher::Error),
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
    #[error("Error while serializing data to file. {0}")]
    Serialization(String),
    #[error("Error writing on spawn blocking file. {0}")]
    SpawnError(#[from] tokio::task::JoinError),
    #[error("Error while committing. {0}")]
    CommitError(#[from] WriteError),
}
