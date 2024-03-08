use std::any::{type_name, Any};
use std::fmt::Debug;
use std::ptr;

use crate::ty::Ty;
use crate::{Eventstore, ProviderGuard};

/// An internal trait that is implemented for any function-like object that can be called once.
pub trait Method<'p, T, P> {
    /// The name of the method. By default is the name of `Self`.
    fn name(&self) -> &'static str {
        type_name::<Self>()
    }
    /// The display name of the method. This is used for printing purposes. It defaults
    /// to the name of the output type.
    fn display_name(&self) -> &'static str {
        type_name::<T>()
    }
    /// Return the events that should be registered after this method is invoked.
    fn events(&self) -> Option<Eventstore> {
        None
    }
    /// The parameters this method will ask for from the registry when invoked.
    fn dependencies(&self) -> Vec<Ty>;
    /// Consume and invoke the method.
    fn call(self, registry: &'p ProviderGuard) -> T;
}
