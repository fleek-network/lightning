//! Infusion is a dependency injection library to help with top-down approach to
//! software development by leveraging traits and generics.
//!
//! See the documentation for [`infu`] for more information. Additional implementation
//! detail notes can also be found in [`note`].
//!
//! You probably want to check out:
//!
//! [`infu`]: The macro that does pretty much everything. Read the documentation on it.
//!
//! [`DependencyGraph`]: This is basically the project model. Each collection implements
//! a `build_graph` function to provide this.
//!
//! [`Container`]: This is where an instance of everything lives in. It owns initialized
//! versions of things. You pass a [`DependencyGraph`] to a container and it initializes
//! all of the objects.

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
