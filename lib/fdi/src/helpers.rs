use std::marker::PhantomData;

use crate::object::Object;
use crate::{Method, Registry};

pub struct Map<F, T, P, M, U> {
    original: F,
    transform: M,
    _p: PhantomData<(T, P, U)>,
}

impl<F, T, P, M, U> Method<U, P> for Map<F, T, P, M, U>
where
    F: Method<T, P>,
    T: 'static,
    M: FnOnce(T) -> U,
    U: 'static,
{
    fn name(&self) -> &'static str {
        self.original.name()
    }

    fn display_name(&self) -> &'static str {
        self.original.display_name()
    }

    fn events(&self) -> Option<crate::Eventstore> {
        self.original.events()
    }

    fn dependencies(&self) -> Vec<crate::ty::Ty> {
        self.original.dependencies()
    }

    fn call(self, registry: &Registry) -> U {
        let value = self.original.call(registry);
        (self.transform)(value)
    }
}

pub struct WithDisplayName<F, T, P> {
    display_name: &'static str,
    method: F,
    _p: PhantomData<(T, P)>,
}

impl<F, T, P> Method<T, P> for WithDisplayName<F, T, P>
where
    F: Method<T, P>,
{
    fn name(&self) -> &'static str {
        self.method.name()
    }

    fn display_name(&self) -> &'static str {
        self.display_name
    }

    fn events(&self) -> Option<crate::Eventstore> {
        self.method.events()
    }

    fn dependencies(&self) -> Vec<crate::ty::Ty> {
        self.method.dependencies()
    }

    fn call(self, registry: &Registry) -> T {
        self.method.call(registry)
    }
}

pub fn to_infalliable<F, T, P>(f: F) -> impl Method<anyhow::Result<T>, P>
where
    F: Method<T, P>,
    T: 'static,
{
    Map {
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
    Map {
        original: f,
        transform: |v| match v {
            Ok(v) => Ok(Object::new(v)),
            Err(e) => Err(e),
        },
        _p: PhantomData,
    }
}
