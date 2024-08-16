use thiserror::Error;

/// Any error returned during different stages of creating and hashing directories.
#[derive(Error, Debug)]
pub enum Error {
    #[error("Provided file name not valid.")]
    InvalidFileName,
    #[error("Provided symbolic link not valid.")]
    InvalidSymlink,
    #[error("Provided ordering of entries was not valid.")]
    InvalidOrdering,
    #[error("File name provided more than once.")]
    FilenameNotUnique,
    #[error("Too many directory items.")]
    TooManyEntries,
}

#[derive(Error, Debug)]
pub enum FromTranscriptError {
    #[error("Transcript data is too small")]
    TranscriptTooSmall,
    #[error("Transcript is invalid")]
    InvalidTranscript,
}
