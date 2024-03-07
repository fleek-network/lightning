mod consume;
pub mod event;
mod executor;
mod ext;
pub mod graph;
mod helpers;
mod impl_tuple;
pub mod method;
pub mod object;
mod rc_cell;
pub mod registry;
pub mod ty;

pub use consume::consume;
pub use event::Eventstore;
pub use executor::Executor;
pub use ext::MethodExt;
pub use graph::DependencyGraph;
pub use method::Method;
pub use object::{Container, Ref, RefMut};
pub use registry::Registry;

#[cfg(test)]
mod tests;
