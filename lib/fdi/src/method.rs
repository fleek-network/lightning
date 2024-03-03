use std::any::{type_name, Any};
use std::marker::PhantomData;
use std::ptr;

use anyhow::Result;

use crate::object::Object;
use crate::registry::Registry;
use crate::ty::Ty;

/// An internal trait that is implemented for any function-like object that can be called once.
pub trait Method<T, P> {
    /// The name of the method. By default is the name of `Self`.
    fn name(&self) -> &'static str {
        type_name::<Self>()
    }
    /// The display name of the method. This is used for printing purposes. It defaults
    /// to the name of the output type.
    fn display_name(&self) -> &'static str {
        type_name::<T>()
    }
    /// The parameters this method will ask for from the registry when invoked.
    fn dependencies(&self) -> Vec<Ty>;
    /// Consume and invoke the method.
    fn call(self, registry: &Registry) -> T;
}

/// A fixed-size struct that can be created from any [`Method`].
pub struct DynMethod {
    tid: Ty,
    name: &'static str,
    display_name: &'static str,
    dependencies: Vec<Ty>,
    ptr: *mut (),
    call_fn: fn(*mut (), registry: &Registry) -> Box<dyn Any>,
    drop_fn: fn(*mut ()),
}

impl DynMethod {
    pub fn new<F, T, P>(method: F) -> Self
    where
        F: Method<T, P>,
        T: 'static,
    {
        let tid = Ty::of::<T>();
        let name = method.name();
        let display_name = method.display_name();

        let dependencies = method.dependencies();

        let value = Box::new(method);
        let ptr = Box::into_raw(value) as *mut ();

        fn call_fn<F, T, P>(ptr: *mut (), registry: &Registry) -> Box<dyn Any>
        where
            F: Method<T, P>,
            T: 'static,
        {
            let ptr = ptr as *mut F;
            let value = unsafe { Box::from_raw(ptr) };
            let result = value.call(registry);
            Box::new(result)
        }

        fn drop_fn<F>(ptr: *mut ()) {
            let ptr = ptr as *mut F;
            let value = unsafe { Box::from_raw(ptr) };
            drop(value);
        }

        Self {
            tid,
            name,
            display_name,
            dependencies,
            ptr,
            call_fn: call_fn::<F, T, P>,
            drop_fn: drop_fn::<F>,
        }
    }

    /// Returns the [`Ty`] of the object that this method will return.
    pub fn ty(&self) -> Ty {
        self.tid
    }

    /// Returns the captured result from [`Method::name`].
    pub fn name(&self) -> &'static str {
        self.name
    }

    /// Returns the captured result from [`Method::display_name`].
    pub fn display_name(&self) -> &'static str {
        self.display_name
    }

    /// Returns the captured result from [`Method::dependencies`].
    pub fn dependencies(&self) -> &Vec<Ty> {
        &self.dependencies
    }

    /// Invoke this method and returns the result in a `Box<dyn Any>`.
    pub fn call(mut self, registry: &Registry) -> Box<dyn Any> {
        // self.construct_fn consumes the pointer so we have to set it to null after use
        // so we wouldn't double-free it on drop.
        let value = (self.call_fn)(self.ptr, registry);
        self.ptr = ptr::null_mut();
        // This should never happen, and only serves as a sanity check.
        assert_eq!(
            value.as_ref().type_id(),
            self.ty().id(),
            "Unexpected return value from method call."
        );
        value
    }
}

impl Drop for DynMethod {
    fn drop(&mut self) {
        // This can happen if construct is not called.
        if !self.ptr.is_null() {
            (self.drop_fn)(self.ptr)
        }
    }
}

/// A higher order function over a method that can change the name of the method to the given name.
pub struct Named<T, P, F: Method<T, P>> {
    name: &'static str,
    method: F,
    _unused: PhantomData<(T, P)>,
}

impl<T, P, F: Method<T, P>> Named<T, P, F> {
    /// Wrap the given method with the given name.
    #[inline(always)]
    pub fn new(name: &'static str, method: F) -> Self {
        Self {
            name,
            method,
            _unused: PhantomData,
        }
    }

    // Wrap the given method with a name parsed from the return type, with all generics removed.
    #[inline(always)]
    pub fn from_return_type(method: F) -> Self {
        let existing = std::any::type_name::<T>();
        if let Some(idx) = existing.find('<') {
            let name = &existing[0..idx];
            Self::new(name, method)
        } else {
            Self::new(existing, method)
        }
    }
}

impl<T, P, F: Method<T, P>> Method<T, P> for Named<T, P, F> {
    #[inline(always)]
    fn name(&self) -> &'static str {
        self.method.name()
    }

    #[inline(always)]
    fn display_name(&self) -> &'static str {
        self.name
    }

    #[inline(always)]
    fn dependencies(&self) -> Vec<Ty> {
        self.method.dependencies()
    }

    #[inline(always)]
    fn call(self, registry: &Registry) -> T {
        self.method.call(registry)
    }
}

/// A method that returns the given value when called.
pub struct Value<T>(pub T);

impl<T> Method<T, ()> for Value<T> {
    #[inline(always)]
    fn dependencies(&self) -> Vec<Ty> {
        vec![]
    }
    fn call(self, _registry: &Registry) -> T {
        self.0
    }
}

pub struct ToInfallible<F>(pub F);
impl<F, T, P> Method<Result<T>, P> for ToInfallible<F>
where
    F: Method<T, P>,
{
    #[inline(always)]
    fn name(&self) -> &'static str {
        self.0.name()
    }

    #[inline(always)]
    fn display_name(&self) -> &'static str {
        self.0.display_name()
    }

    #[inline(always)]
    fn dependencies(&self) -> Vec<Ty> {
        self.0.dependencies()
    }

    #[inline(always)]
    fn call(self, registry: &Registry) -> Result<T> {
        Ok(self.0.call(registry))
    }
}

pub struct ToResultBoxAny<F: Method<Result<T>, P>, T: 'static, P>(F, PhantomData<(T, P)>);
impl<F, T, P> ToResultBoxAny<F, T, P>
where
    F: Method<Result<T>, P>,
    T: 'static,
{
    pub fn new(value: F) -> Self {
        Self(value, PhantomData)
    }
}
impl<F, T, P> Method<Result<Box<dyn Any>>, P> for ToResultBoxAny<F, T, P>
where
    F: Method<Result<T>, P>,
    T: 'static,
{
    #[inline(always)]
    fn name(&self) -> &'static str {
        self.0.name()
    }

    #[inline(always)]
    fn display_name(&self) -> &'static str {
        self.0.display_name()
    }

    #[inline(always)]
    fn dependencies(&self) -> Vec<Ty> {
        self.0.dependencies()
    }

    #[inline(always)]
    fn call(self, registry: &Registry) -> Result<Box<dyn Any>> {
        match self.0.call(registry) {
            Ok(value) => Ok(Box::new(value)),
            Err(e) => Err(e),
        }
    }
}

pub struct ToResultObject<F: Method<Result<T>, P>, T: 'static, P>(F, PhantomData<(T, P)>);
impl<F, T, P> ToResultObject<F, T, P>
where
    F: Method<Result<T>, P>,
    T: 'static,
{
    pub fn new(value: F) -> Self {
        Self(value, PhantomData)
    }
}
impl<F, T, P> Method<Result<Object>, P> for ToResultObject<F, T, P>
where
    F: Method<Result<T>, P>,
    T: 'static,
{
    #[inline(always)]
    fn name(&self) -> &'static str {
        self.0.name()
    }

    #[inline(always)]
    fn display_name(&self) -> &'static str {
        self.0.display_name()
    }

    #[inline(always)]
    fn dependencies(&self) -> Vec<Ty> {
        self.0.dependencies()
    }

    #[inline(always)]
    fn call(self, registry: &Registry) -> Result<Object> {
        match self.0.call(registry) {
            Ok(value) => Ok(Object::new(value)),
            Err(e) => Err(e),
        }
    }
}

macro_rules! impl_for_fn {
    (
        [$($name:ident)*],
        [ $($name_mut:ident)* ]
    ) => {
        impl<F, T $(, $name)* $(, $name_mut)*> Method<T, (
            (
                $($name_mut,)*
            ),
            (
                $($name,)*
            )
        )> for F
        where
            F: FnOnce(
                $(&mut $name_mut,)*
                $(&$name,)*
            ) -> T,
            $($name: 'static,)*
            $($name_mut: 'static,)*
        {
            fn dependencies(&self) -> Vec<Ty> {
                vec![
                    $(Ty::of::<$name>(),)*
                    $(Ty::of::<$name_mut>(),)*
                ]
            }

            #[allow(unused)]
            fn call(self, registry: &Registry) -> T {
                (self)(
                    $(&mut registry.get_mut::<$name_mut>(),)*
                    $(&registry.get::<$name>(),)*
                )
            }
        }
    };
}

macro_rules! for_each_mut {
    ($($name:ident)*) => {
        impl_for_fn!([$($name)*], []);
        impl_for_fn!([$($name)*], [M0]);
        impl_for_fn!([$($name)*], [M0 M1]);
        impl_for_fn!([$($name)*], [M0 M1 M2]);
        impl_for_fn!([$($name)*], [M0 M1 M2 M3]);
        impl_for_fn!([$($name)*], [M0 M1 M2 M3 M4]);
        impl_for_fn!([$($name)*], [M0 M1 M2 M3 M4 M5]);
        impl_for_fn!([$($name)*], [M0 M1 M2 M3 M4 M5 M6]);
        impl_for_fn!([$($name)*], [M0 M1 M2 M3 M4 M5 M6 M7]);
        impl_for_fn!([$($name)*], [M0 M1 M2 M3 M4 M5 M6 M7 M8]);
        impl_for_fn!([$($name)*], [M0 M1 M2 M3 M4 M5 M6 M7 M8 M9]);
        impl_for_fn!([$($name)*], [M0 M1 M2 M3 M4 M5 M6 M7 M8 M9 MA]);
        impl_for_fn!([$($name)*], [M0 M1 M2 M3 M4 M5 M6 M7 M8 M9 MA MB]);
        impl_for_fn!([$($name)*], [M0 M1 M2 M3 M4 M5 M6 M7 M8 M9 MA MB MC]);
        impl_for_fn!([$($name)*], [M0 M1 M2 M3 M4 M5 M6 M7 M8 M9 MA MB MC MD]);
        impl_for_fn!([$($name)*], [M0 M1 M2 M3 M4 M5 M6 M7 M8 M9 MA MB MC MD ME]);
    };
}

for_each_mut!();
for_each_mut!(A0);
for_each_mut!(A0 A1);
for_each_mut!(A0 A1 A2);
for_each_mut!(A0 A1 A2 A3);
for_each_mut!(A0 A1 A2 A3 A4);
for_each_mut!(A0 A1 A2 A3 A4 A5);
for_each_mut!(A0 A1 A2 A3 A4 A5 A6);
for_each_mut!(A0 A1 A2 A3 A4 A5 A6 A7);
for_each_mut!(A0 A1 A2 A3 A4 A5 A6 A7 A8);
for_each_mut!(A0 A1 A2 A3 A4 A5 A6 A7 A8 A9);
for_each_mut!(A0 A1 A2 A3 A4 A5 A6 A7 A8 A9 AA);
for_each_mut!(A0 A1 A2 A3 A4 A5 A6 A7 A8 A9 AA AB);
for_each_mut!(A0 A1 A2 A3 A4 A5 A6 A7 A8 A9 AA AB AC);
for_each_mut!(A0 A1 A2 A3 A4 A5 A6 A7 A8 A9 AA AB AC AD);
for_each_mut!(A0 A1 A2 A3 A4 A5 A6 A7 A8 A9 AA AB AC AD AE);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dyn_test() {
        #[derive(Debug)]
        struct Thing;
        struct ThingMaker {
            dropped: bool,
            _v: Vec<u8>,
        }
        impl Method<Thing, ()> for ThingMaker {
            fn name(&self) -> &'static str {
                "Hello World"
            }
            fn dependencies(&self) -> Vec<Ty> {
                vec![Ty::of::<Thing>()]
            }
            fn call(self, _registry: &Registry) -> Thing {
                Thing
            }
        }
        impl Drop for ThingMaker {
            fn drop(&mut self) {
                assert!(!self.dropped);
                self.dropped = true;
            }
        }
        let maker = DynMethod::new(ThingMaker {
            dropped: false,
            _v: vec![0, 1, 2],
        });
        assert_eq!(maker.ty(), Ty::of::<Thing>());
        assert_eq!(maker.name(), "Hello World");
        assert_eq!(maker.dependencies(), &vec![Ty::of::<Thing>()]);
        let thing = maker.call(&Registry::default());
        thing.downcast::<Thing>().unwrap();
    }

    #[test]
    fn dyn_constructed_named() {
        #[derive(Debug, PartialEq, Eq)]
        struct Thing(Vec<u8>);
        let maker = DynMethod::new(Named::new("Foo", Value(Thing(vec![0, 1]))));
        assert_eq!(maker.ty(), Ty::of::<Thing>());
        assert_eq!(maker.display_name(), "Foo");
        assert_eq!(maker.dependencies(), &vec![]);
        let thing = maker.call(&Registry::default());
        let thing = *thing.downcast::<Thing>().unwrap();
        assert_eq!(&thing, &Thing(vec![0, 1]));
    }
}
