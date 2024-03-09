use std::any::type_name;
use std::cell::RefCell;
use std::marker::PhantomData;

use futures::Future;

use crate::dyn_method::DynMethod;
use crate::provider::{to_obj, Object};
use crate::{Executor, Method, ProviderGuard};

struct Transform<F, T, P, M, U> {
    display_name: &'static str,
    original: F,
    transform: M,
    _p: PhantomData<(T, P, U)>,
}

struct On<F, T, P> {
    original: F,
    name: &'static str,
    handler: RefCell<Option<DynMethod>>,
    _p: PhantomData<(T, P)>,
}

struct Spawn<'a, F, T, P, U>
where
    F: 'static + Method<'a, P, Output = T> + Sized,
    T: 'static + Future<Output = U>,
{
    method: F,
    _p: PhantomData<(&'a F, T, P, U)>,
}

struct BlockOn<'a, F, T, P, U>
where
    F: 'static + Method<'a, P, Output = T> + Sized,
    T: 'static + Future<Output = U>,
{
    method: F,
    _p: PhantomData<(&'a F, T, P, U)>,
}

impl<'a, F, T, P, M, U> Method<'a, P> for Transform<F, T, P, M, U>
where
    F: Method<'a, P, Output = T>,
    T: 'static,
    M: FnOnce(T) -> U,
    U: 'static,
{
    type Output = U;

    #[inline(always)]
    fn name(&self) -> &'static str {
        self.original.name()
    }

    #[inline(always)]
    fn display_name(&self) -> &'static str {
        self.display_name
    }

    #[inline(always)]
    fn events(&self) -> Option<crate::Eventstore> {
        self.original.events()
    }

    #[inline(always)]
    fn dependencies(&self) -> Vec<crate::ty::Ty> {
        self.original.dependencies()
    }

    #[inline(always)]
    fn call(self, registry: &'a ProviderGuard) -> U {
        let value = self.original.call(registry);
        (self.transform)(value)
    }
}

impl<'a, F, T, P> Method<'a, P> for On<F, T, P>
where
    F: Method<'a, P, Output = T>,
    T: 'static,
{
    type Output = T;

    #[inline(always)]
    fn name(&self) -> &'static str {
        self.original.name()
    }

    #[inline(always)]
    fn display_name(&self) -> &'static str {
        self.original.display_name()
    }

    #[inline(always)]
    fn events(&self) -> Option<crate::Eventstore> {
        let events = self.original.events();
        if let Some(handler) = self.handler.borrow_mut().take() {
            let mut events = events.unwrap_or_default();
            // TODO(qti3e)
            events.insert(self.name, handler);
            Some(events)
        } else {
            events
        }
    }

    #[inline(always)]
    fn dependencies(&self) -> Vec<crate::ty::Ty> {
        self.original.dependencies()
    }

    #[inline(always)]
    fn call(self, registry: &'a ProviderGuard) -> T {
        self.original.call(registry)
    }
}

impl<'a, F, T, P, U> Method<'a, P> for Spawn<'a, F, T, P, U>
where
    F: 'static + Method<'a, P, Output = T> + Sized,
    T: 'static + Future<Output = U>,
{
    type Output = ();

    fn name(&self) -> &'static str {
        self.method.name()
    }

    fn events(&self) -> Option<crate::Eventstore> {
        self.method.events()
    }

    fn dependencies(&self) -> Vec<crate::ty::Ty> {
        self.method.dependencies()
    }

    fn call(self, registry: &'a ProviderGuard) {
        let future = self.method.call(registry);
        let registry = registry.provider().clone();
        let mut executor = registry.get_mut::<Executor>();
        executor.spawn(Box::pin(future));
    }
}

impl<'a, F, T, P, U> Method<'a, P> for BlockOn<'a, F, T, P, U>
where
    F: 'static + Method<'a, P, Output = T> + Sized,
    T: 'static + Future<Output = U>,
    U: 'static,
{
    type Output = U;

    fn name(&self) -> &'static str {
        self.method.name()
    }

    fn display_name(&self) -> &'static str {
        self.method.display_name()
    }

    fn events(&self) -> Option<crate::Eventstore> {
        self.method.events()
    }

    fn dependencies(&self) -> Vec<crate::ty::Ty> {
        self.method.dependencies()
    }

    fn call(self, registry: &'a ProviderGuard) -> U {
        let future = self.method.call(registry);
        futures::executor::block_on(future)
    }
}

pub fn map<'a, F, P, U, M>(f: F, transform: M) -> impl Method<'a, P, Output = U>
where
    F: Method<'a, P>,
    M: FnOnce(F::Output) -> U,
    U: 'static,
{
    Transform {
        display_name: type_name::<U>(),
        original: f,
        transform,
        _p: PhantomData,
    }
}

pub fn to_infalliable<'a, F, T, P>(f: F) -> impl Method<'a, P, Output = anyhow::Result<T>>
where
    F: Method<'a, P, Output = T>,
    T: 'static,
{
    Transform {
        display_name: f.display_name(),
        original: f,
        transform: |v| Ok(v),
        _p: PhantomData,
    }
}

pub fn to_result_object<'a, F, T, P>(f: F) -> impl Method<'a, P, Output = anyhow::Result<Object>>
where
    F: Method<'a, P, Output = anyhow::Result<T>>,
    T: 'static,
{
    Transform {
        display_name: f.display_name(),
        original: f,
        transform: |v| match v {
            Ok(v) => Ok(to_obj(v)),
            Err(e) => Err(e),
        },
        _p: PhantomData,
    }
}

pub fn on<'a, F, T, P, H, Q, A>(
    f: F,
    event: &'static str,
    handler: H,
) -> impl Method<'a, P, Output = T>
where
    F: Method<'a, P, Output = T>,
    T: 'static,
    H: Method<'a, A, Output = Q>,
    Q: 'static,
{
    On {
        original: f,
        name: event,
        handler: RefCell::new(Some(DynMethod::new(handler))),
        _p: PhantomData,
    }
}

pub fn display_name<'a, F, T, P>(name: &'static str, f: F) -> impl Method<'a, P, Output = T>
where
    F: Method<'a, P, Output = T>,
    T: 'static,
{
    Transform {
        display_name: name,
        original: f,
        transform: |v| v,
        _p: PhantomData,
    }
}

pub fn spawn<'a, F, T, P, U>(f: F) -> impl Method<'a, P, Output = ()>
where
    F: 'static + Method<'a, P, Output = T> + Sized,
    T: 'static + Future<Output = U>,
{
    Spawn {
        method: f,
        _p: PhantomData,
    }
}

pub fn block_on<'a, F, T, P, U>(f: F) -> impl Method<'a, P, Output = U>
where
    F: 'static + Method<'a, P, Output = T> + Sized,
    T: 'static + Future<Output = U>,
    U: 'static,
{
    BlockOn {
        method: f,
        _p: PhantomData,
    }
}
