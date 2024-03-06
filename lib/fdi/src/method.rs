use std::any::{type_name, Any};
use std::fmt::Debug;
use std::ptr;

use futures::future::LocalBoxFuture;
use futures::Future;

use crate::registry::Registry;
use crate::ty::Ty;
use crate::Eventstore;

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
    /// Return the events that should be registered after this method is invoked.
    fn events(&self) -> Option<Eventstore> {
        None
    }
    /// The parameters this method will ask for from the registry when invoked.
    fn dependencies(&self) -> Vec<Ty>;
    /// Consume and invoke the method.
    fn call(self, registry: &Registry) -> T;

    fn async_call<U>(self, _registry: &Registry) -> LocalBoxFuture<'static, U>
    where
        T: Future<Output = U>,
        Self: 'static + Sized,
    {
        todo!()
    }
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
    events_fn: fn(*mut ()) -> Option<Eventstore>,
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

        fn events_fn<F, T, P>(ptr: *mut ()) -> Option<Eventstore>
        where
            F: Method<T, P>,
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

impl Debug for DynMethod {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("DynMethod").field(&self.name()).finish()
    }
}

// #[cfg(test)]
// mod tests {
//     use super::*;

//     #[test]
//     fn dyn_test() {
//         #[derive(Debug)]
//         struct Thing;
//         struct ThingMaker {
//             dropped: bool,
//             _v: Vec<u8>,
//         }
//         impl Method<Thing, ()> for ThingMaker {
//             fn name(&self) -> &'static str {
//                 "Hello World"
//             }
//             fn dependencies(&self) -> Vec<Ty> {
//                 vec![Ty::of::<Thing>()]
//             }
//             fn call(self, _registry: &Registry) -> Thing {
//                 Thing
//             }
//         }
//         impl Drop for ThingMaker {
//             fn drop(&mut self) {
//                 assert!(!self.dropped);
//                 self.dropped = true;
//             }
//         }
//         let maker = DynMethod::new(ThingMaker {
//             dropped: false,
//             _v: vec![0, 1, 2],
//         });
//         assert_eq!(maker.ty(), Ty::of::<Thing>());
//         assert_eq!(maker.name(), "Hello World");
//         assert_eq!(maker.dependencies(), &vec![Ty::of::<Thing>()]);
//         let thing = maker.call(&Registry::default());
//         thing.downcast::<Thing>().unwrap();
//     }
// }
