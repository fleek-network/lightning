use std::any::type_name;
use std::fmt::Debug;
use std::ptr;

use crate::ty::Ty;
use crate::{Eventstore, Method, Provider};

/// A fixed-size struct that can be created from any [`Method`].
pub struct DynMethod<T: 'static> {
    name: &'static str,
    dependencies: Vec<Ty>,
    ptr: *mut (),
    call_fn: fn(*mut (), provider: &Provider) -> T,
    drop_fn: fn(*mut ()),
    events_fn: fn(*mut ()) -> Option<Eventstore>,
}

impl<T: 'static> DynMethod<T> {
    pub fn new<F, P>(method: F) -> Self
    where
        F: Method<P, Output = T>,
    {
        let name = type_name::<F>();
        let dependencies = F::dependencies();
        let value = Box::new(method);
        let ptr = Box::into_raw(value) as *mut ();

        fn call_fn<F, P>(ptr: *mut (), provider: &Provider) -> F::Output
        where
            F: Method<P>,
        {
            let ptr = ptr as *mut F;
            let value = unsafe { Box::from_raw(ptr) };
            value.call(provider)
        }

        fn drop_fn<F>(ptr: *mut ()) {
            let ptr = ptr as *mut F;
            let value = unsafe { Box::from_raw(ptr) };
            drop(value);
        }

        fn events_fn<F, P>(ptr: *mut ()) -> Option<Eventstore>
        where
            F: Method<P>,
        {
            let ptr = ptr as *mut F;
            let value = unsafe { Box::from_raw(ptr) };
            let result = value.events();
            std::mem::forget(value);
            result
        }

        Self {
            name,
            dependencies,
            ptr,
            call_fn: call_fn::<F, P>,
            drop_fn: drop_fn::<F>,
            events_fn: events_fn::<F, P>,
        }
    }

    /// Returns the [`Ty`] of the object that this method will return.
    pub fn ty(&self) -> Ty {
        Ty::of::<T>()
    }

    /// Returns the name of the original method.
    pub fn name(&self) -> &'static str {
        self.name
    }

    /// Returns the result from [`Method::events`].
    pub fn events(&self) -> Option<Eventstore> {
        (self.events_fn)(self.ptr)
    }

    /// Returns the captured result from [`Method::dependencies`].
    pub fn dependencies(&self) -> &Vec<Ty> {
        &self.dependencies
    }

    /// Invoke this method and returns the result.
    pub fn call(mut self, provider: &Provider) -> T {
        // self.construct_fn consumes the pointer so we have to set it to null after use
        // so we wouldn't double-free it on drop.
        let value = (self.call_fn)(self.ptr, provider);
        self.ptr = ptr::null_mut();
        value
    }
}

impl<T: 'static> Drop for DynMethod<T> {
    fn drop(&mut self) {
        // This can happen if construct is not called.
        if !self.ptr.is_null() {
            (self.drop_fn)(self.ptr)
        }
    }
}

impl<T: 'static> Debug for DynMethod<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("DynMethod").field(&self.name()).finish()
    }
}

#[cfg(test)]
mod tests {
    use super::DynMethod;
    use crate::{Provider, Ref};

    #[test]
    fn basic_test() {
        let provider = Provider::default();
        provider.insert(String::from("Hello"));

        let method = DynMethod::<String>::new(|a: &String| a.clone());
        let out = method.call(&provider);
        assert_eq!(out, String::from("Hello"));

        let method = DynMethod::<()>::new(|a: &mut String| {
            *a = String::from("World");
        });
        method.call(&provider);

        let method = DynMethod::<String>::new(|a: &String| a.clone());
        let out = method.call(&provider);
        assert_eq!(out, String::from("World"));

        provider.get_mut::<String>();
    }

    #[test]
    #[should_panic]
    fn take_ref_out_cb() {
        let provider = Provider::default();
        provider.insert(String::from("Hello"));

        let method = DynMethod::<Ref<String>>::new(|a: Ref<String>| a);
        let out = method.call(&provider);
        provider.get_mut::<String>();
        drop(out);
    }

    #[test]
    fn take_ref_out_cb_but_drop() {
        let provider = Provider::default();
        provider.insert(String::from("Hello"));

        let method = DynMethod::new(|a: Ref<String>| a);
        let out = method.call(&provider);
        drop(out);
        provider.get_mut::<String>();
    }
}
