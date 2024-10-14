use std::any::Any;
use std::cell::UnsafeCell;
use std::collections::HashMap;
use std::marker::PhantomData;
use std::ops::{Deref, DerefMut};
use std::sync::atomic::{AtomicBool, Ordering};

use parking_lot::lock_api::{RawRwLock as RawRwLockTrait, RawRwLockDowngrade};
use parking_lot::{Mutex, RawRwLock, RwLock};
use triomphe::Arc;

use crate::extractor::Extractor;
use crate::ty::Ty;
use crate::{Eventstore, Executor};

/// The [Provider] is used to store a set of dynamically typed values and provide them on demand
/// to a requester.
///
/// Unless [take](Provider::take) is called the returned value is always just a reference to the
/// actual value and it will stay in the provider. Once a value is taken out of the provider, it
/// is as if it was never inserted in the provider.
///
/// The provider is clone and upon clone it will reference to the same provider. Basically think
/// about it as an `Arc<Provider>`.
///
/// Another point worth mentioning is that a [Provider] is basically an RwLock for each inserted
/// value.
///
/// The references returned from Provider are owned, which means even if the [Provider] itself
/// goes out of scope, the values will still stay alive as long as there is a [Ref] or [RefMut]
/// pointing to the value.
///
/// Based on the intended use case there are no 'safe' APIs over the [Provider], any call could
/// result in a panic in case of an error. Which could be any of the following errors:
///
/// 1. Value not existing in the provider.
/// 2. Inserting the same value twice.
/// 3. Requesting a shared reference to the value in case an exclusive lock is alive on the type.
/// 4. Requesting an exclusive lock on the value in case a shared reference is alive.
#[derive(Clone)]
pub struct Provider {
    values: Arc<RwLock<HashMap<Ty, MapEntry>>>,
}

/// For each type [Ty] we store a RawRwLock and
type MapEntry = (Arc<RawRwLock>, Arc<Object>);
pub(crate) type Object = UnsafeCell<Box<dyn Any>>;

/// An scoped guard over a provider so that we can obtain `&T` or `&mut T` from a provider that
/// live as long as this guard.
pub struct ProviderGuard {
    provider: Provider,
    inner: Mutex<ProviderGuardInner>,
}

/// A provider that only allows access to Send+Sync objects.
#[derive(Clone, Default)]
pub struct MultiThreadedProvider {
    local_provider_created: Arc<AtomicBool>,
    provider: Provider,
}

struct ProviderGuardInner {
    /// For each requested value here we store an `Arc<Object>` of that value to keep the
    /// Object alive as long as this struct is dropped.
    values: Vec<Arc<Object>>,
    /// The RawRwLock of an object for shared data access, upon dropping this object we
    /// unlock_shared these locks.
    shared: Vec<Arc<RawRwLock>>,
    /// Like `shared` but for exclusive `&mut T` access to the objects.
    exclusive: Vec<Arc<RawRwLock>>,
}

/// An owned immutable reference to a value of type `T` obtained from a [Provider::get].
pub struct Ref<T: 'static> {
    entry: MapEntry,
    _t: PhantomData<T>,
}

/// An owned mutable reference to a value of type `T` obtained from a [Provider::get_mut].
pub struct RefMut<T: 'static> {
    entry: MapEntry,
    _t: PhantomData<T>,
}

// SAFETY: The data in `Ref` and `RefMut` is wrapped in an Arc. And the access permission
// to the `UnsafeCell` in Object is only done through a valid RwLock guard permissions.
unsafe impl<T: 'static> Send for Ref<T> where T: Send {}
unsafe impl<T: 'static> Send for RefMut<T> where T: Send {}
unsafe impl<T: 'static> Sync for Ref<T> where T: Send + Sync {}
unsafe impl<T: 'static> Sync for RefMut<T> where T: Send + Sync {}

// SAFETY: Multi-threaded provider only provides access to Send+Sync objects.
unsafe impl Send for MultiThreadedProvider {}
unsafe impl Sync for MultiThreadedProvider {}

impl Provider {
    /// Helper to insert the given value to the provider inline during construction.
    ///
    /// # Panics
    ///
    /// If a value with the same type already exists in the provider.
    #[inline(always)]
    pub fn with<T: 'static>(self, value: T) -> Self {
        self.insert(value);
        self
    }

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
    /// If the value does not exists in the provider or if there is an exclusive lock on the value.
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
    /// If the value does not exists in the provider or if there is any other references to the
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
    /// If the value does not exists in the provider or if there is any other references to the
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

    /// Trigger an event with the given name on the [Eventstore] in this provider.
    pub fn trigger(&self, event: &'static str) -> usize {
        let mut store = self.get_mut::<Eventstore>();
        store.trigger(event, self)
    }
}

impl MultiThreadedProvider {
    /// Helper to insert the given value to the provider inline during construction.
    ///
    /// # Panics
    ///
    /// If a value with the same type already exists in the provider.
    #[inline(always)]
    pub fn with<T: 'static + Send + Sync>(self, value: T) -> Self {
        self.insert(value);
        self
    }

    /// Insert the given value to the provider.
    ///
    /// # Panics
    ///
    /// If a value with the same type already exists in the provider.
    pub fn insert<T: 'static + Send + Sync>(&self, value: T) {
        self.provider.insert(value);
    }

    /// Returns a shared reference for a value of type `T`.
    ///
    /// # Panics
    ///
    /// If the value does not exists in the provider or if there is an exclusive lock on the value.
    pub fn get<T: 'static + Send + Sync>(&self) -> Ref<T> {
        self.provider.get()
    }

    /// Returns a mutable and exclusive reference for a value of type `T`.
    ///
    /// # Panics
    ///
    /// If the value does not exists in the provider or if there is any other references to the
    /// value.
    pub fn get_mut<T: 'static + Send + Sync>(&self) -> Ref<T> {
        self.provider.get()
    }

    /// Take the value of type `T` out of the provider.
    ///
    /// # Panics
    ///
    /// If the value does not exists in the provider or if there is any other references to the
    /// value.
    pub fn take<T: 'static + Send + Sync>(&self) -> T {
        self.provider.take()
    }

    /// Returns a normal [Provider] that does not have any Send+Sync restrictions.
    ///
    /// # Panics
    ///
    /// We only allow creation of only one local provider from a multi-threaded provider. So this
    /// method can only be called once.
    pub fn get_local_provider(&self) -> Provider {
        if self
            .local_provider_created
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::Relaxed)
            .is_err()
        {
            panic!("get_local_provider can only be called once.");
        }
        self.provider.clone()
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

impl<T: 'static> RefMut<T> {
    #[inline]
    pub fn downgrade(self) -> Ref<T> {
        // SAFETY: Since we forget the value in the next line `drop` will not be called twice. So
        // the internal state of the `Arc`s will stay valid.
        let entry = unsafe { std::mem::transmute_copy::<MapEntry, MapEntry>(&self.entry) };
        std::mem::forget(self);

        // SAFETY: We own an exclusive lock right now. So it must be possible and safe to downgrade
        // it to a shared lock.
        unsafe {
            entry.0.downgrade();
        }

        Ref {
            entry,
            _t: PhantomData,
        }
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
