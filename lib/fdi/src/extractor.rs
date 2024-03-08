use crate::provider::{Provider, ProviderGuard, Ref, RefMut};
use crate::ty::Ty;

pub trait Extractor<'a>: 'a {
    fn extract<'p: 'a>(registry: &'p ProviderGuard) -> Self;
    fn dependencies(collector: &mut Vec<Ty>);
}

trait M<T, P> {
    fn call(registry: &Provider) -> T;
}

impl<'a, T: 'static> Extractor<'a> for &'a T {
    fn extract<'p: 'a>(registry: &'p ProviderGuard) -> Self {
        registry.get::<T>()
    }
    fn dependencies(collector: &mut Vec<Ty>) {
        collector.push(Ty::of::<T>())
    }
}

impl<'a, T: 'static> Extractor<'a> for &'a mut T {
    fn extract<'p: 'a>(registry: &'p ProviderGuard) -> Self {
        registry.get_mut::<T>()
    }
    fn dependencies(collector: &mut Vec<Ty>) {
        collector.push(Ty::of::<T>())
    }
}

impl<'a, T: 'static> Extractor<'a> for Ref<T> {
    fn extract<'p: 'a>(registry: &'p ProviderGuard) -> Self {
        registry.provider().get::<T>()
    }
    fn dependencies(collector: &mut Vec<Ty>) {
        collector.push(Ty::of::<T>())
    }
}

impl<'a, T: 'static> Extractor<'a> for RefMut<T> {
    fn extract<'p: 'a>(registry: &'p ProviderGuard) -> Self {
        registry.provider().get_mut::<T>()
    }
    fn dependencies(collector: &mut Vec<Ty>) {
        collector.push(Ty::of::<T>())
    }
}

impl<F, T, A0> M<T, (A0,)> for F
where
    F: FnOnce(A0) -> T,
    A0: for<'a> Extractor<'a>,
{
    fn call(registry: &Provider) -> T {
        todo!()
    }
}

fn expect_method<F, T, P>(f: F)
where
    F: M<T, P>,
{
    todo!()
}

fn usage() {
    expect_method(|a: Ref<String>| {});
}
