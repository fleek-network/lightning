use std::any::type_name;

use crate::ty::Ty;
use crate::{Eventstore, Provider};

/// An internal trait that is implemented for any function-like object that can be called once.
pub trait Method<P> {
    type Output: 'static;

    /// The name of the method. By default is the name of `Self`.
    fn name(&self) -> &'static str {
        type_name::<Self>()
    }

    /// The display name of the method. This is used for printing purposes. It defaults
    /// to the name of the output type.
    fn display_name(&self) -> &'static str {
        type_name::<Self::Output>()
    }

    /// Return the events that should be registered after this method is invoked.
    fn events(&self) -> Option<Eventstore> {
        None
    }

    /// The parameters this method will ask for from the registry when invoked.
    fn dependencies() -> Vec<Ty>;

    /// Consume and invoke the method.
    fn call(self, registry: &Provider) -> Self::Output;
}
