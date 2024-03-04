use std::any::type_name;
use std::marker::PhantomData;

use crate::object::Object;
use crate::{Method, Registry};

struct Transform<F, T, P, M, U> {
    display_name: &'static str,
    original: F,
    transform: M,
    _p: PhantomData<(T, P, U)>,
}

impl<F, T, P, M, U> Method<U, P> for Transform<F, T, P, M, U>
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
        self.display_name
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

pub fn to_infalliable<F, T, P>(f: F) -> impl Method<anyhow::Result<T>, P>
where
    F: Method<T, P>,
    T: 'static,
{
    Transform {
        display_name: type_name::<anyhow::Result<T>>(),
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
