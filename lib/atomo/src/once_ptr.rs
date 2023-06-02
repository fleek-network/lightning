use std::sync::atomic::AtomicPtr;

/// An atomic pointer that can only be initialized once.
pub struct OncePtr<T>(AtomicPtr<T>);

impl<T> OncePtr<T> {
    /// Create a new uninitialized pointer.
    #[inline]
    pub fn null() -> Self {
        Self(AtomicPtr::new(std::ptr::null_mut()))
    }

    /// Create a new initialized pointer for the given data.
    #[inline]
    pub fn new(value: T) -> Self {
        let ptr = Box::into_raw(Box::new(value));
        Self(AtomicPtr::new(ptr))
    }

    /// Initialize the store with the provided value.
    ///
    /// # Panics
    ///
    /// If the store is already initialized before.
    #[inline]
    pub fn store(&self, value: T) {
        let pointer = Box::into_raw(Box::new(value));
        let previous = self.0.swap(pointer, std::sync::atomic::Ordering::Acquire);
        assert!(previous.is_null(), "Store can only be called once.");
    }

    /// Returns true if the store is not initialized and is null.
    #[inline]
    pub fn is_null(&self) -> bool {
        let ptr = self.0.load(std::sync::atomic::Ordering::Relaxed);
        ptr.is_null()
    }

    /// Load the atomic store and return a reference to the underlying data or [`None`]
    /// if the store is not initialized yet.
    #[inline]
    pub fn load(&self) -> Option<&T> {
        let ptr = self.0.load(std::sync::atomic::Ordering::Relaxed);
        if ptr.is_null() {
            None
        } else {
            Some(unsafe { &*ptr })
        }
    }

    /// Load the atomic store and return a reference to the underlying data without
    /// checking if it's null.
    ///
    /// # Safety
    ///
    /// It is up to the caller to ensure that the pointer is not null.
    #[inline]
    pub unsafe fn load_unchecked(&self) -> &T {
        let ptr = self.0.load(std::sync::atomic::Ordering::Relaxed);
        unsafe { &*ptr }
    }

    /// Load the atomic store and return a mutable reference to the underlying data or
    /// [`None`] if the store is not initialized yet.
    ///
    /// This is safe because the mutable reference guarantees that no other threads are
    /// concurrently accessing the atomic data.
    #[inline]
    pub fn load_mut(&mut self) -> Option<&mut T> {
        let ptr = *self.0.get_mut();
        if ptr.is_null() {
            None
        } else {
            Some(unsafe { &mut *ptr })
        }
    }

    /// Load the atomic store and return a mutable reference to the underlying data
    /// without checking if it's null.
    ///
    /// # Safety
    ///
    /// It is up to the caller to ensure that the pointer is not null.
    #[inline]
    pub unsafe fn load_mut_unchecked(&mut self) -> &mut T {
        let ptr = *self.0.get_mut();
        unsafe { &mut *ptr }
    }

    /// Returns the data owned by this store.
    #[inline]
    pub fn into_inner(mut self) -> Option<T> {
        let ptr = self.0.get_mut();
        if ptr.is_null() {
            None
        } else {
            let ptr = std::mem::replace(ptr, std::ptr::null_mut());
            Some(*unsafe { Box::from_raw(ptr) })
        }
    }
}

impl<T> Drop for OncePtr<T> {
    fn drop(&mut self) {
        let ptr = *self.0.get_mut();
        if !ptr.is_null() {
            // SAFETY: We own the data.
            unsafe {
                drop(Box::from_raw(ptr));
            }
        }
    }
}
