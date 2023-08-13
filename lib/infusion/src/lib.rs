//! Infusion is a dependency injection library to help with top-down approach to
//! software development by leveraging traits and generics.
//!
//! See the documentation for [`infu`] for more information. Additional implementation
//! detail notes can also be found in [`note`].
//!
//! ## Main Components
//!
//! There are 3 main parts that you should read about.
//!
//! | NAME     | Description |
//! | -------- | ----------- |
//! | [`infu`] | The main macro to generate a collection and service traits. |
//! | [`DependencyGraph`] | The concrete project model generated by a collection. |
//! | [`Container`] | The DI-provider. This is where instances of objects live. |
//!
//! ## Utility macros
//!
//! There are also some utility macros:
//!
//! | NAME     | Description |
//! | -------- | ----------- |
//! | [`p`]    | This can be used to help with accessing the type on a collection. |
//! | [`tag`]  | This can be used to help generate a [`Tag`](vtable::Tag). |
//! | [`ok`]  | Can be used to create an infallible `Ok`. |
//!
//! # Example
//!
//! ```
#![doc = include_str!("../examples/graph.rs")]
//! ```

#![allow(clippy::all, unused)]

/// The implementation of macros.
mod macros;

/// The error types.
pub mod error;

/// Notes on the implementation detail.
pub mod note;

/// The implementation of the virtual table and dynamic object.
pub mod vtable;

/// The implementation of the container.
pub mod container;

/// Project graph.
pub mod graph;

pub use container::Container;
pub use error::InitializationError;
pub use graph::DependencyGraph;
