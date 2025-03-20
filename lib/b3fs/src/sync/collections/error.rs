use thiserror::Error;

/// An error returned during the conversion from a slice.
#[derive(Error, Debug)]
pub enum CollectionTryFromError {
    #[error("Length of the provided slice was not a multiple of 32.")]
    InvalidNumberOfBytes,
    #[error("Length of the provided hash array was not forming a valid tree.")]
    InvalidHashCount,
}
