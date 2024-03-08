mod consume;
mod event;
mod executor;
mod ext;
mod extractor;
mod graph;
mod helpers;
mod impl_tuple;
mod lab;
mod method;
mod provider;
mod ty;

pub use event::Eventstore;
pub use executor::Executor;
pub use ext::MethodExt;
pub use graph::DependencyGraph;
pub use method::Method;
pub use provider::{Provider, ProviderGuard, Ref, RefMut};

// #[cfg(test)]
// mod tests;

// mod tests {
//     use crate::DependencyGraph;

//     fn x() {
//         #[derive(Default)]
//         struct A;

//         let graph = DependencyGraph::new().with_infallible(|x: &mut String| A::default());
//     }
// }
