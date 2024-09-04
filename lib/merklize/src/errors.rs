use thiserror::Error;

use crate::StateRootHash;

#[derive(Error, Debug)]
pub enum StateTreeError {
    #[error("state root mismatch (expected {expected:?}, found {stored:?})")]
    VerifyStateTreeError {
        expected: StateRootHash,
        stored: StateRootHash,
    },

    #[error("state tree error: {0}")]
    Unknown(String),
}

impl From<anyhow::Error> for StateTreeError {
    fn from(error: anyhow::Error) -> Self {
        StateTreeError::Unknown(error.to_string())
    }
}

impl From<String> for StateTreeError {
    fn from(error: String) -> Self {
        StateTreeError::Unknown(error)
    }
}
