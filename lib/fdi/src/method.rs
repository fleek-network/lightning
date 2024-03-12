use crate::ty::Ty;
use crate::{Eventstore, Provider};

/// An internal trait that is implemented for any function-like object that can be called once.
pub trait Method<P> {
    type Output;

    /// The display name of the method. This is used for printing purposes. It defaults
    /// to the name of the output type.
    fn display_name(&self) -> Option<String>;

    /// Return the events that should be registered after this method is invoked.
    fn events(&self) -> Option<Eventstore>;

    /// The parameters this method will ask for from the provider when invoked.
    fn dependencies() -> Vec<Ty>;

    /// Consume and invoke the method.
    fn call(self, provider: &Provider) -> Self::Output;
}
