use crate::provider::{ProviderGuard, Ref, RefMut};
use crate::ty::Ty;

pub trait Extractor<'a>: 'a {
    fn extract<'p: 'a>(provider: &'p ProviderGuard) -> Self;
    fn dependencies(collector: &mut Vec<Ty>);
}

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
