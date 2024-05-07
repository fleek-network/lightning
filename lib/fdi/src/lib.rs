//! # Fdi
//!
//! This is a very simple dependency injection system focusing on initialization of a large system.
//!
//! It allows you to describe a dependency graph by providing a list of constructor functions for
//! different values. To do that you should use [DependencyGraph] to add a series of constructor
//! functions.

mod bind;
mod dyn_method;
mod event;
mod executor;
mod ext;
mod extractor;
mod graph;
mod helpers;
mod impl_tuple;
mod method;
mod provider;
mod ty;
pub mod viz;

pub use bind::{consume, Bind, Captured};
pub use event::Eventstore;
pub use executor::Executor;
pub use ext::MethodExt;
pub use extractor::{Cloned, Consume, Extractor};
pub use graph::{BuildGraph, DependencyGraph};
pub use method::Method;
pub use provider::{MultiThreadedProvider, Provider, ProviderGuard, Ref, RefMut};

#[cfg(test)]
mod tests;
