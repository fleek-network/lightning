#![allow(clippy::all, unused)]

/// The implementation of macros.
mod macros;

mod error;

/// Notes on the implementation detail.
pub mod note;

/// The implementation of the virtual table and dynamic object.
pub mod vtable;

/// The implementation of the container.
pub mod container;

pub use container::{Container, DependencyGraph};
pub use error::ContainerError;
