use std::sync::Arc;

use arc_swap::ArcSwap;

use crate::once_ptr::OncePtr;

/// A [`GcList`] is responsible for moving the time of snapshots forward in
/// an atomic way. It achieves this by maintaining a reference counted list
/// items of type `T`, however the value of the head of the list can be `null`
/// as well, but we guarantee that we only set the value of that pointer once
/// through the use of [`OncePtr`].
// TODO(qti3e): Rewrite the following data structure without the use of ArcSwap.
pub(crate) struct GcList<T> {
    head: ArcSwap<GcNode<T>>,
}

pub(crate) struct GcNode<T> {
    pub value: OncePtr<T>,
    pub next: OncePtr<Arc<GcNode<T>>>,
}

impl<T> GcList<T> {
    /// Create a new empty [`GcList`] instance.
    pub fn new() -> Self {
        Self {
            head: ArcSwap::new(Arc::new(GcNode::new())),
        }
    }

    /// # Safety
    ///
    /// Only one thread should be calling this function, it is the responsibility
    /// of the caller to make sure that's the case.
    ///
    /// You must also ensure that the returned value from this function is immediately
    /// passed to [`Self::set_head`]. There must not be another call to `push` until
    /// `set_head` is called.
    pub unsafe fn push(&self, value: T) -> Arc<GcNode<T>> {
        let next = Arc::new(GcNode::new());
        let head = self.head.load();
        head.set_value(value);
        head.set_next(next.clone());
        next
    }

    /// # Safety
    ///
    /// The `next` provided here must be obtained from a previous call to `push`.
    pub unsafe fn set_head(&self, next: Arc<GcNode<T>>) {
        self.head.store(next);
    }

    /// Returns the current head.
    pub fn current(&self) -> Arc<GcNode<T>> {
        self.head.load().clone()
    }
}

impl<T> GcNode<T> {
    pub fn new() -> Self {
        GcNode {
            value: OncePtr::null(),
            next: OncePtr::null(),
        }
    }

    pub fn with_value_and_next(value: T, next: Arc<GcNode<T>>) -> Self {
        GcNode {
            value: OncePtr::new(value),
            next: OncePtr::new(next),
        }
    }

    pub fn into_value(self) -> Option<T> {
        self.value.into_inner()
    }

    /// # Panics
    ///
    /// If the entries is already set.
    #[inline]
    pub fn set_value(&self, entries: T) {
        self.value.store(entries);
    }

    #[inline]
    pub fn set_next(&self, next: Arc<GcNode<T>>) {
        self.next.store(next);
    }
}
