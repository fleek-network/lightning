use std::any::type_name;
use std::cell::RefCell;
use std::marker::PhantomData;

use futures::Future;

use crate::dyn_method::DynMethod;
use crate::provider::{to_obj, Object};
use crate::ty::Ty;
use crate::{Eventstore, Executor, Method, Provider};

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

struct Spawn<F, T, P, U>
where
    F: 'static + Method<P, Output = T> + Sized,
    T: 'static + Future<Output = U>,
{
    method: F,
    _p: PhantomData<(F, T, P, U)>,
}

struct BlockOn<F, T, P, U>
where
    F: 'static + Method<P, Output = T> + Sized,
    T: 'static + Future<Output = U>,
{
    method: F,
    _p: PhantomData<(F, T, P, U)>,
}

struct Flatten<F, A, B> {
    method: F,
    _p: PhantomData<(A, B)>,
}

impl<F, T, P, M, U> Method<P> for Transform<F, T, P, M, U>
where
    F: Method<P, Output = T>,
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
    fn dependencies() -> Vec<Ty> {
        F::dependencies()
    }

    #[inline(always)]
    fn call(self, registry: &Provider) -> U {
        let value = self.original.call(registry);
        (self.transform)(value)
    }
}

impl<F, T, P> Method<P> for On<F, T, P>
where
    F: Method<P, Output = T>,
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
            events.insert(self.name, handler);
            Some(events)
        } else {
            events
        }
    }

    #[inline(always)]
    fn dependencies() -> Vec<Ty> {
        F::dependencies()
    }

    #[inline(always)]
    fn call(self, registry: &Provider) -> T {
        self.original.call(registry)
    }
}

impl<F, T, P, U> Method<P> for Spawn<F, T, P, U>
where
    F: 'static + Method<P, Output = T> + Sized,
    T: 'static + Future<Output = U>,
{
    type Output = ();

    #[inline(always)]
    fn name(&self) -> &'static str {
        self.method.name()
    }

    #[inline(always)]
    fn events(&self) -> Option<crate::Eventstore> {
        self.method.events()
    }

    #[inline(always)]
    fn dependencies() -> Vec<Ty> {
        F::dependencies()
    }

    #[inline(always)]
    fn call(self, registry: &Provider) {
        let future = self.method.call(registry);
        let mut executor = registry.get_mut::<Executor>();
        executor.spawn(Box::pin(future));
    }
}

impl<F, T, P, U> Method<P> for BlockOn<F, T, P, U>
where
    F: 'static + Method<P, Output = T> + Sized,
    T: 'static + Future<Output = U>,
    U: 'static,
{
    type Output = U;

    #[inline(always)]
    fn name(&self) -> &'static str {
        self.method.name()
    }

    #[inline(always)]
    fn display_name(&self) -> &'static str {
        self.method.display_name()
    }

    #[inline(always)]
    fn events(&self) -> Option<crate::Eventstore> {
        self.method.events()
    }

    #[inline(always)]
    fn dependencies() -> Vec<Ty> {
        F::dependencies()
    }

    #[inline(always)]
    fn call(self, registry: &Provider) -> U {
        let future = self.method.call(registry);
        futures::executor::block_on(future)
    }
}

impl<F, A, B> Method<(A, B)> for Flatten<F, A, B>
where
    F: Method<A>,
    F::Output: Method<B>,
{
    type Output = <F::Output as Method<B>>::Output;

    #[inline(always)]
    fn name(&self) -> &'static str {
        self.method.name()
    }

    #[inline(always)]
    fn display_name(&self) -> &'static str {
        self.method.display_name()
    }

    #[inline(always)]
    fn events(&self) -> Option<crate::Eventstore> {
        self.method.events()
    }

    #[inline(always)]
    fn dependencies() -> Vec<Ty> {
        let mut out = F::dependencies();
        out.extend(F::Output::dependencies());
        out
    }

    #[inline(always)]
    fn call(self, registry: &Provider) -> Self::Output {
        let inner_method = self.method.call(registry);

        if let Some(events) = inner_method.events() {
            let mut event_store = registry.get_mut::<Eventstore>();
            event_store.extend(events);
        }

        inner_method.call(registry)
    }
}

#[inline(always)]
pub fn map<F, P, U, M>(f: F, transform: M) -> impl Method<P, Output = U>
where
    F: Method<P>,
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

#[inline(always)]
pub fn to_infalliable<F, T, P>(f: F) -> impl Method<P, Output = anyhow::Result<T>>
where
    F: Method<P, Output = T>,
    T: 'static,
{
    Transform {
        display_name: f.display_name(),
        original: f,
        transform: |v| Ok(v),
        _p: PhantomData,
    }
}

#[inline(always)]
pub fn to_result_object<F, T, P>(f: F) -> impl Method<P, Output = anyhow::Result<Object>>
where
    F: Method<P, Output = anyhow::Result<T>>,
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

#[inline(always)]
pub fn on<F, T, P, H, Q, A>(f: F, event: &'static str, handler: H) -> impl Method<P, Output = T>
where
    F: Method<P, Output = T>,
    T: 'static,
    H: Method<A, Output = Q>,
    Q: 'static,
{
    On {
        original: f,
        name: event,
        handler: RefCell::new(Some(DynMethod::new(handler))),
        _p: PhantomData,
    }
}

#[inline(always)]
pub fn display_name<F, T, P>(name: &'static str, f: F) -> impl Method<P, Output = T>
where
    F: Method<P, Output = T>,
    T: 'static,
{
    Transform {
        display_name: name,
        original: f,
        transform: |v| v,
        _p: PhantomData,
    }
}

#[inline(always)]
pub fn spawn<F, T, P, U>(f: F) -> impl Method<P, Output = ()>
where
    F: 'static + Method<P, Output = T> + Sized,
    T: 'static + Future<Output = U>,
{
    Spawn {
        method: f,
        _p: PhantomData,
    }
}

#[inline(always)]
pub fn block_on<F, T, P, U>(f: F) -> impl Method<P, Output = U>
where
    F: 'static + Method<P, Output = T> + Sized,
    T: 'static + Future<Output = U>,
    U: 'static,
{
    BlockOn {
        method: f,
        _p: PhantomData,
    }
}

#[inline(always)]
pub fn flatten<F, A, B>(f: F) -> impl Method<(A, B), Output = <F::Output as Method<B>>::Output>
where
    F: Method<A>,
    F::Output: Method<B>,
{
    Flatten::<F, A, B> {
        method: f,
        _p: PhantomData,
    }
}
