use thiserror::Error;

use crate::verifier::IncrementalVerifierError;

#[derive(Error, Debug)]
pub enum PutFeedProofError {
    #[error("Putter was running without incremental verification.")]
    UnexpectedCall,
    #[error("Proof is not matching the current root.")]
    InvalidProof(#[from] IncrementalVerifierError),
}

#[derive(Error, Debug)]
pub enum PutWriteError {
    #[error("The provided content is not matching the hash.")]
    InvalidContent,
    #[error("The provided content could not be decompressed.")]
    DecompressionFailure,
}

#[derive(Error, Debug)]
pub enum PutInsertError {
    #[error("The provided entry does not meet the expected hash.")]
    InvalidContent,
    #[error("The provided entry violates the entry name ordering.")]
    OrderingError,
}

#[derive(Error, Debug)]
pub enum PutFinalizeError {
    #[error("The putter expected more data to come to a finalized state.")]
    PartialContent,
    #[error("The final CID does not match the CID which was expected.")]
    InvalidCID,
    #[error("Writing to disk failed.")]
    WriteFailed,
}
