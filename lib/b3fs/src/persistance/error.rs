use thiserror::Error;

use crate::directory::error::Error as HasherError;

#[derive(Debug, Error)]
#[error(transparent)]
pub enum PersistanceError {
    /// The error happened with the hasher while trying to hash the data.
    Hasher(HasherError),
    /// The error originated from writing the bytes to the target buffer.
    Writer(std::io::Error),
}

impl From<HasherError> for PersistanceError {
    fn from(value: HasherError) -> Self {
        Self::Hasher(value)
    }
}

impl From<std::io::Error> for PersistanceError {
    fn from(e: std::io::Error) -> Self {
        Self::Writer(e)
    }
}
