use thiserror::Error;

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
}
