use std::ops::{Deref, DerefMut};

use crate::provider::{ProviderGuard, Ref, RefMut};
use crate::ty::{Ownership, Param, Ty};

/// This trait could be implemented for any type that we want to pass to a method by computing
/// something over data that is already in a provider.
pub trait Extractor<'a>: 'a {
    /// Given a reference to a [ProviderGuard] must return an instance of Self.
    fn extract<'p: 'a>(provider: &'p ProviderGuard) -> Self;

    /// Should return the types expected to be in the provider when [extract](Extractor::extract)
    /// is called.
    fn dependencies(collector: &mut Vec<Param>);
}

/// An extractor that can clone a value of type `T` from the provider.
pub struct Cloned<T: 'static + Clone>(pub T);

/// An extractor that can take a value of type `T` out of the provider.
pub struct Consume<T: 'static>(pub T);

impl<'a, T: 'static> Extractor<'a> for &'a T {
    #[inline(always)]
    fn extract<'p: 'a>(provider: &'p ProviderGuard) -> Self {
        provider.get::<T>()
    }
    fn dependencies(collector: &mut Vec<Param>) {
        collector.push((Ownership::Ref, Ty::of::<T>()))
    }
}

impl<'a, T: 'static> Extractor<'a> for &'a mut T {
    #[inline(always)]
    fn extract<'p: 'a>(provider: &'p ProviderGuard) -> Self {
        provider.get_mut::<T>()
    }
    fn dependencies(collector: &mut Vec<Param>) {
        collector.push((Ownership::RefMut, Ty::of::<T>()))
    }
}

impl<'a, T: 'static> Extractor<'a> for Ref<T> {
    #[inline(always)]
    fn extract<'p: 'a>(provider: &'p ProviderGuard) -> Self {
        provider.provider().get::<T>()
    }
    fn dependencies(collector: &mut Vec<Param>) {
        collector.push((Ownership::Ref, Ty::of::<T>()))
    }
}

impl<'a, T: 'static> Extractor<'a> for RefMut<T> {
    #[inline(always)]
    fn extract<'p: 'a>(provider: &'p ProviderGuard) -> Self {
        provider.provider().get_mut::<T>()
    }
    fn dependencies(collector: &mut Vec<Param>) {
        collector.push((Ownership::RefMut, Ty::of::<T>()))
    }
}

impl<'a, T: 'static + Clone> Extractor<'a> for Cloned<T> {
    #[inline(always)]
    fn extract<'p: 'a>(provider: &'p ProviderGuard) -> Self {
        let g = provider.provider().get::<T>();
        Self(g.clone())
    }
    fn dependencies(collector: &mut Vec<Param>) {
        collector.push((Ownership::Ref, Ty::of::<T>()))
    }
}

impl<'a, T: 'static> Extractor<'a> for Consume<T> {
    #[inline(always)]
    fn extract<'p: 'a>(provider: &'p ProviderGuard) -> Self {
        Self(provider.provider().take::<T>())
    }
    fn dependencies(collector: &mut Vec<Param>) {
        collector.push((Ownership::Take, Ty::of::<T>()))
    }
}

impl<'a, T: 'static> Extractor<'a> for triomphe::Arc<T> {
    #[inline(always)]
    fn extract<'p: 'a>(provider: &'p ProviderGuard) -> Self {
        provider.provider().get::<triomphe::Arc<T>>().clone()
    }
    fn dependencies(collector: &mut Vec<Param>) {
        collector.push((Ownership::Ref, Ty::of::<triomphe::Arc<T>>()))
    }
}

impl<'a, T: 'static> Extractor<'a> for std::sync::Arc<T> {
    #[inline(always)]
    fn extract<'p: 'a>(provider: &'p ProviderGuard) -> Self {
        provider.provider().get::<std::sync::Arc<T>>().clone()
    }
    fn dependencies(collector: &mut Vec<Param>) {
        collector.push((Ownership::Ref, Ty::of::<std::sync::Arc<T>>()));
    }
}

impl<'a, T: 'static> Extractor<'a> for std::rc::Rc<T> {
    #[inline(always)]
    fn extract<'p: 'a>(provider: &'p ProviderGuard) -> Self {
        provider.provider().get::<std::rc::Rc<T>>().clone()
    }
    fn dependencies(collector: &mut Vec<Param>) {
        collector.push((Ownership::Ref, Ty::of::<std::rc::Rc<T>>()))
    }
}

impl<T: 'static + Clone> Deref for Cloned<T> {
    type Target = T;
    #[inline(always)]
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<T: 'static + Clone> DerefMut for Cloned<T> {
    #[inline(always)]
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl<T: 'static> Deref for Consume<T> {
    type Target = T;
    #[inline(always)]
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<T: 'static> DerefMut for Consume<T> {
    #[inline(always)]
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}
