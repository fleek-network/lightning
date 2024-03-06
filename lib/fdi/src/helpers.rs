use std::cell::RefCell;
use std::marker::PhantomData;

use futures::Future;

use crate::method::DynMethod;
use crate::object::Object;
use crate::{Executor, Method, Registry};

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

struct Wrap<F, T, P, W, U, A, R> {
    method: F,
    wrapper: W,
    _p: PhantomData<(F, T, P, W, U, A, R)>,
}

struct Spawn<F, T, P, U>
where
    F: 'static + Method<T, P> + Sized,
    T: 'static + Future<Output = U>,
{
    method: F,
    _p: PhantomData<(F, T, P, U)>,
}

struct BlockOn<F, T, P, U>
where
    F: 'static + Method<T, P> + Sized,
    T: 'static + Future<Output = U>,
{
    method: F,
    _p: PhantomData<(F, T, P, U)>,
}

impl<F, T, P, M, U> Method<U, P> for Transform<F, T, P, M, U>
where
    F: Method<T, P>,
    T: 'static,
    M: FnOnce(T) -> U,
    U: 'static,
{
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
    fn call(self, registry: &Registry) -> U {
        let value = self.original.call(registry);
        (self.transform)(value)
    }
}

impl<F, T, P> Method<T, P> for On<F, T, P>
where
    F: Method<T, P>,
    T: 'static,
{
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
    fn call(self, registry: &Registry) -> T {
        self.original.call(registry)
    }
}

impl<F, T, P, W, U, A, R> Method<R, (P, A)> for Wrap<F, T, P, W, U, A, R>
where
    F: Method<T, P>,
    T: 'static,
    W: Method<U, A>,
    U: 'static + FnOnce(T) -> R,
{
    fn dependencies(&self) -> Vec<crate::ty::Ty> {
        let mut dep = self.method.dependencies();
        dep.extend(self.wrapper.dependencies());
        dep
    }

    fn call(self, registry: &Registry) -> R {
        let result = self.method.call(registry);
        let higher = self.wrapper.call(registry);
        (higher)(result)
    }
}

impl<F, T, P, U> Method<(), P> for Spawn<F, T, P, U>
where
    F: 'static + Method<T, P> + Sized,
    T: 'static + Future<Output = U>,
{
    fn name(&self) -> &'static str {
        self.method.name()
    }

    fn events(&self) -> Option<crate::Eventstore> {
        self.method.events()
    }

    fn dependencies(&self) -> Vec<crate::ty::Ty> {
        self.method.dependencies()
    }

    fn call(self, registry: &Registry) {
        let mut executor = registry.get_mut::<Executor>();
        let registry = registry.snapshot();
        executor.spawn(Box::pin(async move {
            self.method.call(&registry).await;
        }));
    }
}

impl<F, T, P, U> Method<U, P> for BlockOn<F, T, P, U>
where
    F: 'static + Method<T, P> + Sized,
    T: 'static + Future<Output = U>,
{
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

    fn call(self, registry: &Registry) -> U {
        let future = self.method.call(registry);
        futures::executor::block_on(future)
    }
}

pub fn to_infalliable<F, T, P>(f: F) -> impl Method<anyhow::Result<T>, P>
where
    F: Method<T, P>,
    T: 'static,
{
    Transform {
        display_name: f.display_name(),
        original: f,
        transform: |v| Ok(v),
        _p: PhantomData,
    }
}

pub fn to_result_object<F, T, P>(f: F) -> impl Method<anyhow::Result<Object>, P>
where
    F: Method<anyhow::Result<T>, P>,
    T: 'static,
{
    Transform {
        display_name: f.display_name(),
        original: f,
        transform: |v| match v {
            Ok(v) => Ok(Object::new(v)),
            Err(e) => Err(e),
        },
        _p: PhantomData,
    }
}

pub fn on<F, T, P, H, Q, A>(f: F, event: &'static str, handler: H) -> impl Method<T, P>
where
    F: Method<T, P>,
    T: 'static,
    H: Method<Q, A>,
    Q: 'static,
{
    On {
        original: f,
        name: event,
        handler: RefCell::new(Some(DynMethod::new(handler))),
        _p: PhantomData,
    }
}

pub fn display_name<F, T, P>(name: &'static str, f: F) -> impl Method<T, P>
where
    F: Method<T, P>,
    T: 'static,
{
    Transform {
        display_name: name,
        original: f,
        transform: |v| v,
        _p: PhantomData,
    }
}

pub fn wrap<F, T, P, W, U, A, R>(f: F, w: W) -> impl Method<R, (P, A)>
where
    F: Method<T, P>,
    T: 'static,
    W: Method<U, A>,
    U: 'static + FnOnce(T) -> R,
{
    Wrap {
        method: f,
        wrapper: w,
        _p: PhantomData,
    }
}

pub fn spawn<F, T, P, U>(f: F) -> impl Method<(), P>
where
    F: 'static + Method<T, P> + Sized,
    T: 'static + Future<Output = U>,
{
    Spawn {
        method: f,
        _p: PhantomData,
    }
}

pub fn block_on<F, T, P, U>(f: F) -> impl Method<U, P>
where
    F: 'static + Method<T, P> + Sized,
    T: 'static + Future<Output = U>,
{
    BlockOn {
        method: f,
        _p: PhantomData,
    }
}
