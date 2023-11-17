/// The implementation of macros.
mod macros2;

/// The error types.
pub mod error;

/// The implementation of the virtual table and dynamic object.
pub mod object;

/// The implementation of the container.
pub mod container;

/// Project graph.
pub mod graph;
pub mod handler;
pub mod registry;

pub use container::Container;
pub use error::InitializationError;
pub use graph::DependencyGraph;
pub use infusion_proc::{blank, service};

mod blank;

pub use blank::Blank;
#[doc(hidden)]
pub use infusion_proc::{__blank_helper, __gen_macros_helper, __modifier_helper};
#[doc(hidden)]
pub use paste::paste;
