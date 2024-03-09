use std::any::Any;
use std::cell::UnsafeCell;
use std::collections::HashMap;
use std::marker::PhantomData;
use std::ops::{Deref, DerefMut};

use parking_lot::lock_api::RawRwLock as RawRwLockTrait;
use parking_lot::{Mutex, RawRwLock, RwLock};
use triomphe::Arc;

use crate::extractor::Extractor;
use crate::ty::Ty;
use crate::{Eventstore, Executor};

#[derive(Clone)]
pub struct Provider {
    values: Arc<RwLock<HashMap<Ty, MapEntry>>>,
}

type MapEntry = (Arc<RawRwLock>, Arc<Object>);
pub(crate) type Object = UnsafeCell<Box<dyn Any>>;

pub struct ProviderGuard {
    provider: Provider,
    inner: Mutex<ProviderGuardInner>,
}

struct ProviderGuardInner {
    values: Vec<Arc<Object>>,
    shared: Vec<Arc<RawRwLock>>,
    exclusive: Vec<Arc<RawRwLock>>,
}

pub struct Ref<T: 'static> {
    entry: MapEntry,
    _t: PhantomData<T>,
}

pub struct RefMut<T: 'static> {
    entry: MapEntry,
    _t: PhantomData<T>,
}

impl Provider {
    /// Insert the given value to the provider.
    ///
    /// # Panics
    ///
    /// If a value with the same type already exists in the provider.
    pub fn insert<T: 'static>(&self, value: T) {
        let obj = UnsafeCell::new(Box::new(value));
        self.insert_raw(Ty::of::<T>(), obj);
    }

    pub(crate) fn insert_raw(&self, ty: Ty, obj: Object) {
        let mut guard = self.values.write();
        if guard.contains_key(&ty) {
            panic!(
                "Value for type '{}' already exists in the provider.",
                ty.name()
            );
        }
        guard.insert(ty, (Arc::new(RawRwLock::INIT), Arc::new(obj)));
    }

    /// Returns a [ProviderGuard] from this [Provider]. The guard could be used to resolve
    /// references that live as long as the guard.
    pub fn guard(&self) -> ProviderGuard {
        ProviderGuard::new(self.clone())
    }

    /// Returns true if the provider contains a value for the given type.
    pub fn contains<T: 'static>(&self) -> bool {
        self.contains_ty(&Ty::of::<T>())
    }

    /// Returns true if the provider contains a value for the given type.
    pub fn contains_ty(&self, ty: &Ty) -> bool {
        let guard = self.values.read();
        guard.contains_key(ty)
    }

    /// Returns a shared reference for a value of type `T`.
    ///
    /// # Panics
    ///
    /// If the value does not exists in the registry or if there is an exclusive lock on the value.
    pub fn get<T: 'static>(&self) -> Ref<T> {
        let guard = self.values.read();
        Ref {
            entry: get_value(&guard, &Ty::of::<T>(), true),
            _t: PhantomData,
        }
    }

    /// Returns a mutable and exclusive reference for a value of type `T`.
    ///
    /// # Panics
    ///
    /// If the value does not exists in the registry or if there is any other references to the
    /// value.
    pub fn get_mut<T: 'static>(&self) -> RefMut<T> {
        let guard = self.values.read();
        RefMut {
            entry: get_value(&guard, &Ty::of::<T>(), false),
            _t: PhantomData,
        }
    }

    /// Take the value of type `T` out of the provider.
    ///
    /// # Panics
    ///
    /// If the value does not exists in the registry or if there is any other references to the
    /// value.
    pub fn take<T: 'static>(&self) -> T {
        let ty = Ty::of::<T>();
        let mut guard = self.values.write();

        {
            // Acquire an exclusive lock on the value. And forget about the lock and the Arc
            // reference to the value.
            get_value(&guard, &ty, false);
        }

        // Now that we have an exclusive guard on the value we can remove it from this provider.
        let (_, value) = guard.remove(&ty).unwrap();
        let unsafe_cell = Arc::try_unwrap(value).unwrap();
        let any_box = unsafe_cell.into_inner();
        *any_box.downcast::<T>().unwrap()
    }

    pub fn trigger(&self, event: &'static str) -> usize {
        let mut store = self.get_mut::<Eventstore>();
        store.trigger(event, self)
    }
}

impl ProviderGuard {
    fn new(provider: Provider) -> Self {
        Self {
            provider,
            inner: Mutex::new(ProviderGuardInner {
                values: Vec::new(),
                shared: Vec::new(),
                exclusive: Vec::new(),
            }),
        }
    }

    /// Returns a reference to the provider under this guard.
    pub fn provider(&self) -> &Provider {
        &self.provider
    }

    /// Returns a shared reference to a value of type `T`.
    ///
    /// # Panics
    ///
    /// If the provider does not contain the value or if there is an exclusive access to the value.
    pub fn get<T: 'static>(&self) -> &T {
        let guard = self.provider.values.read();
        let ty = Ty::of::<T>();
        let (rw, arc) = get_value(&guard, &ty, true);
        let ptr = arc.get();

        // Remember the lock and the value's Arc.
        let mut mutex_guard = self.inner.lock();
        mutex_guard.values.push(arc);
        mutex_guard.shared.push(rw);

        unsafe { &*ptr }.downcast_ref::<T>().unwrap()
    }

    /// Returns an exclusive reference to a value of type `T`.
    ///
    /// # Panics
    ///
    /// If the provider does not contain the value or if there is any other references to the value.
    #[allow(clippy::mut_from_ref)]
    pub fn get_mut<T: 'static>(&self) -> &mut T {
        let guard = self.provider.values.read();
        let ty = Ty::of::<T>();
        let (rw, arc) = get_value(&guard, &ty, false);
        let ptr = arc.get();

        // Remember the lock and the value's Arc.
        let mut mutex_guard = self.inner.lock();
        mutex_guard.values.push(arc);
        mutex_guard.exclusive.push(rw);

        unsafe { &mut *ptr }.downcast_mut::<T>().unwrap()
    }

    /// Extract the given value from this guard.
    pub fn extract<'s, 'e, E>(&'s self) -> E
    where
        E: Extractor<'e>,
        's: 'e,
    {
        E::extract(self)
    }
}

impl<T: 'static> Deref for Ref<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        let inner = unsafe { &*self.entry.1.get() };
        inner.downcast_ref::<T>().unwrap()
    }
}

impl<T: 'static> Deref for RefMut<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        let inner = unsafe { &*self.entry.1.get() };
        inner.downcast_ref::<T>().unwrap()
    }
}

impl<T: 'static> DerefMut for RefMut<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        let inner = unsafe { &mut *self.entry.1.get() };
        inner.downcast_mut::<T>().unwrap()
    }
}

impl<T: 'static> Drop for Ref<T> {
    fn drop(&mut self) {
        unsafe {
            self.entry.0.unlock_shared();
        }
    }
}

impl<T: 'static> Drop for RefMut<T> {
    fn drop(&mut self) {
        unsafe {
            self.entry.0.unlock_exclusive();
        }
    }
}

impl Drop for ProviderGuard {
    fn drop(&mut self) {
        let inner = self.inner.lock();

        for rw in &inner.shared {
            unsafe { rw.unlock_shared() }
        }

        for rw in &inner.exclusive {
            unsafe { rw.unlock_exclusive() }
        }
    }
}

impl Default for Provider {
    fn default() -> Self {
        let provider = Provider {
            values: Default::default(),
        };
        provider.insert(Eventstore::default());
        provider.insert(Executor::default());
        provider
    }
}

fn get_value(map: &HashMap<Ty, MapEntry>, ty: &Ty, shared: bool) -> MapEntry {
    let entry = map
        .get(ty)
        .unwrap_or_else(|| {
            panic!(
                "Could not get the value with type '{}' from the provider.",
                ty.name()
            )
        })
        .clone();

    if shared {
        if !entry.0.try_lock_shared() {
            panic!(
                "Could not get read access for value with type '{}'",
                ty.name()
            );
        }
    } else if !entry.0.try_lock_exclusive() {
        panic!(
            "Could not get write access for value with type '{}'.",
            ty.name()
        );
    }

    entry
}

pub(crate) fn to_obj<T: 'static>(value: T) -> Object {
    UnsafeCell::new(Box::new(value))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn usage() {
        struct A(String);
        struct B(String);
        let provider = Provider::default();
        provider.insert(A(String::from("Hello")));
        provider.insert(B(String::from("Hello")));

        fn f(a: &A, b: &B) {}

        let guard = provider.guard();
        let a = guard.get::<A>();
        let b = guard.get::<B>();
        // drop(guard);
        // let c = guard.get_mut::<B>();
        // let d = guard.get_mut::<A>();
        f(a, b);
    }
}
