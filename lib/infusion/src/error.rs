use std::fmt::{Debug, Display};

use crate::vtable::Tag;

/// A cycle was found. Contains two path one from `u -> v` and another
/// from `v -> u`.
#[derive(Debug)]
pub struct CycleFound(pub Vec<Tag>, pub Vec<Tag>);

/// The error returned from the initialization of a container.
#[derive(Debug)]
pub enum ContainerError {
    CycleFound(CycleFound),

    /// Attempt to initialize a member failed and an error was returned.
    InitializationFailed(Tag, Box<dyn std::error::Error>),

    /// The required input was not provided.
    InputNotProvided(Tag),
}

impl Display for CycleFound {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // TODO: Print the two path.
        write!(f, "Found a cycle in the dependency graph.")
    }
}

impl Display for ContainerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ContainerError::CycleFound(_) => {
                write!(f, "Cycle Found")
            },
            ContainerError::InitializationFailed(tag, _) => {
                // No need to print the error because we implement `source`.
                write!(f, "Initialization of '{tag:?}' failed.")
            },
            ContainerError::InputNotProvided(tag) => {
                write!(f, "Required value for '{tag:?}' was not provided.")
            },
        }
    }
}

impl std::error::Error for CycleFound {}

impl std::error::Error for ContainerError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            ContainerError::CycleFound(err) => Some(err),
            ContainerError::InitializationFailed(_, err) => Some(err.as_ref()),
            ContainerError::InputNotProvided(_) => None,
        }
    }
}
