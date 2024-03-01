use std::any::{type_name, Any, TypeId};
use std::cell::{self, RefCell, UnsafeCell};
use std::collections::HashMap;
use std::marker::PhantomData;
use std::ops::{Deref, DerefMut};
use std::rc::Rc;

use crate::event::Eventstore;
use crate::ty::Ty;

/// The registry contains all of the constructed values.
///
/// # Technical Notes
///
/// The registry is meant to be used during an overall initlization of a system and allowing it to
/// be [`Sync`] or [`Send`] would allow non optimal use cases. So it doesn't implement those.
pub struct Registry<T = ()>(Rc<UnsafeCell<InnerMap>>, PhantomData<T>);
type InnerMap = HashMap<Ty, RefCell<Option<Box<dyn Any>>>>;

/// For when the registry is mutable.
#[derive(Default)]
pub struct MutMarker;

pub type MutRegistry = Registry<MutMarker>;

pub struct Ref<'a, T: ?Sized + 'a>(cell::Ref<'a, T>);
pub struct RefMut<'a, T: ?Sized + 'a>(cell::RefMut<'a, T>);
pub struct RegistryGuard<'a>(Registry, PhantomData<&'a Registry>);

pub struct Taker<T: 'static>(RefCell<Option<T>>);
impl<T: 'static> Taker<T> {}

impl Registry<MutMarker> {
    fn get_map_mut(&mut self) -> &mut InnerMap {
        unsafe { &mut *self.0.get() }
    }

    /// Insert the given value to the registry.
    pub fn insert<T: 'static>(&mut self, value: T) {
        self.insert_raw(Ty::of::<T>(), Box::new(value));
    }

    pub fn insert_raw(&mut self, tid: Ty, value: Box<dyn Any>) {
        if tid == Ty::of::<Registry>() || tid == Ty::of::<MutRegistry>() {
            panic!("Adding registry to registry is not safe and disabled.");
        }
        self.get_map_mut().insert(tid, RefCell::new(Some(value)));
    }

    pub fn as_reader(&mut self) -> RegistryGuard<'_> {
        RegistryGuard(Registry::<()>(self.0.clone(), PhantomData), PhantomData)
    }
}

impl From<Registry<MutMarker>> for Registry<()> {
    fn from(value: Registry<MutMarker>) -> Self {
        Self(value.0, PhantomData)
    }
}

impl<U> Registry<U> {
    /// Tries to convert a registry into a mutable registry.
    pub fn try_into_mut(self) -> Option<Registry<MutMarker>> {
        // ensure this is the only version of the registry that is alive.
        if Rc::strong_count(&self.0) == 1 && Rc::weak_count(&self.0) == 0 {
            Some(Registry::<MutMarker>(self.0, PhantomData))
        } else {
            None
        }
    }

    fn get_map(&self) -> &InnerMap {
        // SAFETY: Registry is not Send nor Sync. So it does not leave the main thread.
        unsafe { &*self.0.get() }
    }

    #[inline(always)]
    fn map_get<T: 'static>(&self) -> &RefCell<Option<Box<dyn Any>>> {
        let tid = Ty::of::<T>();
        self.get_map().get(&tid).unwrap_or_else(|| {
            panic!(
                "No value for type '{}' found in the registry.",
                type_name::<T>()
            )
        })
    }

    /// Returns true if the registry has a value with the given type.
    pub fn contains<T: 'static>(&self) -> bool {
        let tid = Ty::of::<T>();
        self.get_map().contains_key(&tid)
    }

    pub fn contains_type_id(&self, tid: &Ty) -> bool {
        self.get_map().contains_key(tid)
    }

    pub fn get<T: 'static + Any>(&self) -> Ref<'_, T> {
        Ref(cell::Ref::map(
            self.map_get::<T>().try_borrow().unwrap_or_else(|e| {
                panic!(
                    "Could not get a ref to the value of type '{}' from registry: {e}",
                    type_name::<T>()
                )
            }),
            |x| {
                x.as_ref()
                    .unwrap_or_else(|| {
                        panic!(
                            "Value of type '{}' has been taken out from the registry.",
                            type_name::<T>()
                        )
                    })
                    .downcast_ref::<T>()
                    .unwrap()
            },
        ))
    }

    pub fn get_mut<T: 'static + Any>(&self) -> RefMut<'_, T> {
        RefMut(cell::RefMut::map(
            self.map_get::<T>().try_borrow_mut().unwrap_or_else(|e| {
                panic!(
                    "Could not get a mutable ref to the value of type '{}' from registry: {e}",
                    type_name::<T>()
                )
            }),
            |x| {
                x.as_mut()
                    .unwrap_or_else(|| {
                        panic!(
                            "Value of type '{}' has been taken out from the registry.",
                            type_name::<T>()
                        )
                    })
                    .downcast_mut::<T>()
                    .unwrap()
            },
        ))
    }

    /// Take the value of the given type out of the registry.
    pub fn take<T: 'static>(&self) -> T {
        let mut x = self.map_get::<T>().try_borrow_mut().unwrap_or_else(|e| {
            panic!(
                "Could not get a mutable ref to the value of type '{}' from registry: {e}",
                type_name::<T>()
            )
        });
        *x.take()
            .unwrap_or_else(|| {
                panic!(
                    "Value of type '{}' has been taken out from the registry.",
                    type_name::<T>()
                )
            })
            .downcast::<T>()
            .unwrap()
    }

    /// Trigger the event with the given name from the event store.
    pub fn trigger(&self, event: &'static str) -> usize {
        let registry = Registry::<()>(self.0.clone(), PhantomData);
        self.get_mut::<Eventstore>().trigger(event, &registry)
    }
}

impl Clone for Registry<()> {
    fn clone(&self) -> Self {
        Self(self.0.clone(), PhantomData)
    }
}

impl<T> Default for Registry<T> {
    fn default() -> Self {
        let mut registry = Registry::<MutMarker>(Default::default(), PhantomData);
        registry.insert(Eventstore::default());
        Registry::<T>(registry.0, PhantomData)
    }
}

impl<'a, T: ?Sized + 'a> Deref for Ref<'a, T> {
    type Target = T;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<'a, T: ?Sized + 'a> Deref for RefMut<'a, T> {
    type Target = T;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<'a, T: ?Sized + 'a> DerefMut for RefMut<'a, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl<'a> Deref for RegistryGuard<'a> {
    type Target = Registry;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
