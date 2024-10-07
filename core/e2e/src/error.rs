use lightning_utils::poll::PollUntilError;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum SwarmError {
    #[error("Timeout waiting for swarm to be ready")]
    WaitForReadyTimeout,

    #[error("Internal: {0}")]
    Internal(String),
}

impl From<PollUntilError> for SwarmError {
    fn from(error: PollUntilError) -> Self {
        match error {
            PollUntilError::Timeout => SwarmError::WaitForReadyTimeout,
            PollUntilError::ConditionError(_) => SwarmError::Internal(error.to_string()),
            PollUntilError::ConditionNotSatisfied => unreachable!(),
        }
    }
}
