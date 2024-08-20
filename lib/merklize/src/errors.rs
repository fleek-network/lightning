use crate::StateRootHash;

#[derive(Debug)]
pub enum VerifyStateTreeError {
    StateRootMismatch(StateRootHash, StateRootHash),
}

impl std::fmt::Display for VerifyStateTreeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            VerifyStateTreeError::StateRootMismatch(expected, actual) => write!(
                f,
                "state root mismatch: expected={} actual={}",
                expected, actual
            ),
        }
    }
}

impl std::error::Error for VerifyStateTreeError {}
