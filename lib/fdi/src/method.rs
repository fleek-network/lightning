use crate::ty::Param;
use crate::{Eventstore, Provider};

/// An internal trait that is implemented for any function-like object that can be called once.
pub trait Method<P> {
    type Output;

    /// Return the events that should be registered after this method is invoked.
    fn events(&self) -> Option<Eventstore>;

    /// The parameters this method will ask for from the provider when invoked.
    fn dependencies() -> Vec<Param>;

    /// Consume and invoke the method.
    fn call(self, provider: &Provider) -> Self::Output;
}
