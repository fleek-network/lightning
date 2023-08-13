/// Re-export infallible since we want to use in the macros.
pub use std::convert::Infallible;
use std::fmt::{Debug, Display};

use crate::vtable::Tag;

/// A cycle is reported as a path starting from a node `v` and ending with `v`.
pub type Cycle = Vec<Tag>;

/// One or more cycle was found. There is always at least one element in this
/// vec.
#[derive(Debug)]
pub struct CycleFound(pub Vec<Cycle>);

/// The error returned from the initialization of a container.
#[derive(Debug)]
pub enum InitializationError {
    CycleFound(CycleFound),

    /// Attempt to initialize a member failed and an error was returned.
    InitializationFailed(Tag, Box<dyn std::error::Error>),

    /// The required input was not provided.
    InputNotProvided(Tag),
}

impl Display for CycleFound {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // TODO: Print the cycles.
        write!(f, "Found a cycle in the dependency graph.")
    }
}

impl Display for InitializationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            InitializationError::CycleFound(_) => {
                write!(f, "Cycle Found")
            },
            InitializationError::InitializationFailed(tag, _) => {
                // No need to print the error because we implement `source`.
                write!(f, "Initialization of '{tag:?}' failed.")
            },
            InitializationError::InputNotProvided(tag) => {
                write!(f, "Required value for '{tag:?}' was not provided.")
            },
        }
    }
}

impl std::error::Error for CycleFound {}

impl std::error::Error for InitializationError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            InitializationError::CycleFound(err) => Some(err),
            InitializationError::InitializationFailed(_, err) => Some(err.as_ref()),
            InitializationError::InputNotProvided(_) => None,
        }
    }
}
