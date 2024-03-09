use std::ops::{Deref, DerefMut};

use crate::provider::{ProviderGuard, Ref, RefMut};
use crate::ty::Ty;

pub trait Extractor<'a>: 'a {
    fn extract<'p: 'a>(provider: &'p ProviderGuard) -> Self;
    fn dependencies(collector: &mut Vec<Ty>);
}

pub struct Cloned<T: 'static + Clone>(pub T);

pub struct Consume<T: 'static>(pub T);

impl<'a, T: 'static> Extractor<'a> for &'a T {
    fn extract<'p: 'a>(provider: &'p ProviderGuard) -> Self {
        provider.get::<T>()
    }
    fn dependencies(collector: &mut Vec<Ty>) {
        collector.push(Ty::of::<T>())
    }
}

impl<'a, T: 'static> Extractor<'a> for &'a mut T {
    fn extract<'p: 'a>(provider: &'p ProviderGuard) -> Self {
        provider.get_mut::<T>()
    }
    fn dependencies(collector: &mut Vec<Ty>) {
        collector.push(Ty::of::<T>())
    }
}

impl<'a, T: 'static> Extractor<'a> for Ref<T> {
    fn extract<'p: 'a>(provider: &'p ProviderGuard) -> Self {
        provider.provider().get::<T>()
    }
    fn dependencies(collector: &mut Vec<Ty>) {
        collector.push(Ty::of::<T>())
    }
}

impl<'a, T: 'static> Extractor<'a> for RefMut<T> {
    fn extract<'p: 'a>(provider: &'p ProviderGuard) -> Self {
        provider.provider().get_mut::<T>()
    }
    fn dependencies(collector: &mut Vec<Ty>) {
        collector.push(Ty::of::<T>())
    }
}

impl<'a, T: 'static + Clone> Extractor<'a> for Cloned<T> {
    fn extract<'p: 'a>(provider: &'p ProviderGuard) -> Self {
        let g = provider.provider().get::<T>();
        Self(g.clone())
    }
    fn dependencies(collector: &mut Vec<Ty>) {
        collector.push(Ty::of::<T>())
    }
}

impl<'a, T: 'static> Extractor<'a> for Consume<T> {
    fn extract<'p: 'a>(provider: &'p ProviderGuard) -> Self {
        Self(provider.provider().take::<T>())
    }
    fn dependencies(collector: &mut Vec<Ty>) {
        collector.push(Ty::of::<T>())
    }
}

impl<T: 'static + Clone> Deref for Cloned<T> {
    type Target = T;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<T: 'static + Clone> DerefMut for Cloned<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl<T: 'static> Deref for Consume<T> {
    type Target = T;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<T: 'static> DerefMut for Consume<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}
