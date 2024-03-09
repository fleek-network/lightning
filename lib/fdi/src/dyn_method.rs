use std::any::Any;
use std::fmt::Debug;
use std::ptr;

use crate::ty::Ty;
use crate::{Eventstore, Method, ProviderGuard};

/// A fixed-size struct that can be created from any [`Method`].
pub struct DynMethod {
    tid: Ty,
    name: &'static str,
    display_name: &'static str,
    dependencies: Vec<Ty>,
    ptr: *mut (),
    call_fn: fn(*mut (), registry: ProviderGuard) -> Box<dyn Any>,
    drop_fn: fn(*mut ()),
    events_fn: fn(*mut ()) -> Option<Eventstore>,
}

impl DynMethod {
    pub fn new<'a, F, T, P>(method: F) -> Self
    where
        F: Method<'a, P, Output = T>,
        T: 'static,
    {
        let tid = Ty::of::<T>();
        let name = method.name();
        let display_name = method.display_name();

        let dependencies = method.dependencies();

        let value = Box::new(method);
        let ptr = Box::into_raw(value) as *mut ();

        fn call_fn<'a, F, T, P>(ptr: *mut (), guard: ProviderGuard) -> Box<dyn Any>
        where
            F: Method<'a, P, Output = T>,
            T: 'static,
        {
            let ptr = ptr as *mut F;
            let value = unsafe { Box::from_raw(ptr) };
            let guard = Box::into_raw(Box::new(guard));
            let result = {
                let guard = unsafe { &*guard };
                value.call(guard)
            };
            unsafe {
                drop(Box::from_raw(guard));
            };
            Box::new(result)
        }

        fn drop_fn<F>(ptr: *mut ()) {
            let ptr = ptr as *mut F;
            let value = unsafe { Box::from_raw(ptr) };
            drop(value);
        }

        fn events_fn<'a, F, T, P>(ptr: *mut ()) -> Option<Eventstore>
        where
            F: Method<'a, P, Output = T>,
            T: 'static,
        {
            let ptr = ptr as *mut F;
            let value = unsafe { Box::from_raw(ptr) };
            let result = value.events();
            std::mem::forget(value);
            result
        }

        Self {
            tid,
            name,
            display_name,
            dependencies,
            ptr,
            call_fn: call_fn::<F, T, P>,
            drop_fn: drop_fn::<F>,
            events_fn: events_fn::<F, T, P>,
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

    /// Returns the result from [`Method::events`].
    pub fn events(&self) -> Option<Eventstore> {
        (self.events_fn)(self.ptr)
    }

    /// Returns the captured result from [`Method::dependencies`].
    pub fn dependencies(&self) -> &Vec<Ty> {
        &self.dependencies
    }

    /// Invoke this method and returns the result in a `Box<dyn Any>`.
    pub fn call(mut self, registry: ProviderGuard) -> Box<dyn Any> {
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

impl Debug for DynMethod {
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

        let method = DynMethod::new(|a: &String| a.clone());
        let guard = provider.guard();
        let out = *method.call(guard).downcast::<String>().unwrap();
        assert_eq!(out, String::from("Hello"));

        let method = DynMethod::new(|a: &mut String| {
            *a = String::from("World");
        });
        let guard = provider.guard();
        method.call(guard);

        let method = DynMethod::new(|a: &String| a.clone());
        let guard = provider.guard();
        let out = *method.call(guard).downcast::<String>().unwrap();
        assert_eq!(out, String::from("World"));

        provider.get_mut::<String>();
    }

    #[test]
    #[should_panic]
    fn take_ref_out_cb() {
        let provider = Provider::default();
        provider.insert(String::from("Hello"));

        let method = DynMethod::new(|a: Ref<String>| a);
        let guard = provider.guard();
        let out = *method.call(guard).downcast::<Ref<String>>().unwrap();
        provider.get_mut::<String>();
        drop(out);
    }

    #[test]
    fn take_ref_out_cb_but_drop() {
        let provider = Provider::default();
        provider.insert(String::from("Hello"));

        let method = DynMethod::new(|a: Ref<String>| a);
        let guard = provider.guard();
        let out = *method.call(guard).downcast::<Ref<String>>().unwrap();
        drop(out);
        provider.get_mut::<String>();
    }

    #[test]
    fn xxx() {
        let provider = Provider::default();
        provider.insert(String::from("Hello"));

        fn get_static_str(s: &'static String) -> &'static String {
            println!("= {s}");
            s
        }

        let method = DynMethod::new(|a: &'static String| get_static_str(a));
        let guard = provider.guard();
        let out = *method.call(guard).downcast::<&'static String>().unwrap();

        let method = DynMethod::new(|a: &mut String| {
            *a = String::from("World");
        });
        let guard = provider.guard();
        method.call(guard);

        println!("out = {out}");
        drop(provider.take::<String>());
        println!("out = {out}");
    }
}
