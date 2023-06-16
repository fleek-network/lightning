use std::{
    num::NonZeroU64,
    ptr::null_mut,
    sync::atomic::{AtomicPtr, AtomicUsize, Ordering},
};

use seize::{reclaim, Collector, Linked};

use crate::once_ptr::OncePtr;

/// The snapshot list data structure is used to maintain a concurrent and garbage-collected
/// linked-list of snapshots.
///
/// The idea of this data structure comes from the fact that the first snapshot only contains
/// a delta and points directly to the persistence layer.
///
/// Once a new snapshot is created, we change the previous head's 'next' to be equal to the
/// new node. This is a normal linked-list. The interesting part is the garbage collection.
///
/// This list only provides one function to get a shared reference-counted pointer to the
/// head of the list (i.e the latest head), there is no method to perform random-access or
/// getting the tail. And we want to remove the oldest nodes when they are no longer pointed
/// to.
///
/// Each node in this linked-list (i.e `SnapshotInner`) keeps a 'counter' which increases
/// when the most recent item is accessed and decreased when the pointer ([`Snapshot`]) is
/// removed.
///
/// In this implementation we use [`seize`] for memory reclamation and only retire objects
/// when a new item is pushed to the list. So when a [`Snapshot`] is dropped nothing more
/// than an atomic number decrease happens.
///
/// And when a new item is pushed we go from the tail of the list and `retire` objects as
/// long as they are not referenced anymore and let [`seize`] take care of physically
/// removing them.
// TODO: This can even further improved since seize is an overkill. What we really need
// is a much simpler reclamation.
pub struct SnapshotList<T, U> {
    collector: Collector,
    head: AtomicPtr<Linked<SnapshotInner<T, U>>>,
    tail: AtomicPtr<Linked<SnapshotInner<T, U>>>,
}

impl<T, U> Default for SnapshotList<T, U>
where
    U: Default,
{
    fn default() -> Self {
        Self::new(U::default())
    }
}

impl<T, U> SnapshotList<T, U> {
    /// Create a new empty snapshot list.
    pub fn new(metadata: U) -> Self {
        let collector = Collector::new()
            // By default `seize` waits for 120 objects to get retired before it attempts
            // to remove things. But that's too much for us since batches might not be
            // really small. We definitely need to pick a lower number, but we have to
            // find it in practice. Right now I *guess* 16 might be a good start.
            .batch_size(16)
            // Default value in `seize` is 110. This is the number newly linked items before
            // an epoch is progressed. For correctness this should be less than the batch_size
            // for the purposes of this linked-list.
            .epoch_frequency(NonZeroU64::new(10));

        let node = collector.link_boxed(SnapshotInner::empty(metadata));

        SnapshotList {
            collector,
            head: AtomicPtr::new(node),
            tail: AtomicPtr::new(node),
        }
    }

    /// Push a new data into the list. This sets the data of the current head to the provided
    /// `batch` and creates a new empty head. The transition function is called right before we
    /// move to the newly generated head. (before `self.head = new_head.`).
    ///
    /// # Safety
    ///
    /// This function can not be called concurrently. Only one thread can access it at a time.
    pub fn push<F>(&self, data: T, metadata: U, transition: F)
    where
        F: FnOnce(),
    {
        let guard = self.collector.enter();

        // Create the new head.
        let new_head = self.collector.link_boxed(SnapshotInner::empty(metadata));

        // Update the previous head's inner data.
        let prev_head_ptr = guard.protect(&self.head, Ordering::Acquire);
        debug_assert!(!prev_head_ptr.is_null());
        let prev_head = unsafe { &*prev_head_ptr };
        prev_head.data.store(data);
        prev_head.next.store(new_head, Ordering::Relaxed);

        // Perform the transition function that should be called in between.
        transition();

        // Set the new head.
        self.head.store(new_head, Ordering::Relaxed);

        // Now let's retire the oldest snapshots.
        let mut tail = self.tail.load(Ordering::Relaxed);
        let original_tail = tail;

        loop {
            if tail == prev_head_ptr {
                break;
            }

            debug_assert!(!tail.is_null());
            let tail_ref = unsafe { &*tail };
            let counter = tail_ref.counter.load(Ordering::Relaxed);

            if counter > 0 {
                // there is at least one [`Snapshot`] that is still around.
                break;
            }

            unsafe {
                self.collector
                    .retire(tail, reclaim::boxed::<SnapshotInner<T, U>>);
            }

            // move to the next node and loop again.
            tail = tail_ref.next.load(Ordering::Relaxed);
        }

        if tail != original_tail {
            self.tail.store(tail, Ordering::Relaxed);
        }
    }

    /// Returns the current snapshot for the data.
    pub fn current(&self) -> Snapshot<T, U> {
        let guard = self.collector.enter();
        let head_ptr = guard.protect(&self.head, Ordering::Acquire);
        Snapshot::new(head_ptr)
    }
}

/// The snapshot object.
pub struct Snapshot<T, U> {
    inner: *mut Linked<SnapshotInner<T, U>>,
}

struct SnapshotInner<T, U> {
    counter: AtomicUsize,
    next: AtomicPtr<Linked<SnapshotInner<T, U>>>,
    data: OncePtr<T>,
    metadata: U,
}

impl<T, U> SnapshotInner<T, U> {
    /// Create a new empty snapshot inner with everything set to null
    /// and the reference counter set to zero.
    #[inline(always)]
    pub fn empty(metadata: U) -> Self {
        SnapshotInner {
            counter: AtomicUsize::new(0),
            next: AtomicPtr::new(null_mut()),
            data: OncePtr::null(),
            metadata,
        }
    }
}

impl<T, U> Snapshot<T, U> {
    /// Create a new snapshot.
    #[inline(always)]
    fn new(inner_ptr: *mut Linked<SnapshotInner<T, U>>) -> Self {
        debug_assert!(!inner_ptr.is_null());
        let inner = unsafe { &*inner_ptr };
        inner.counter.fetch_add(1, Ordering::Relaxed);
        Self { inner: inner_ptr }
    }

    /// Returns the meta data associated with the current snapshot.
    pub fn get_metadata(&self) -> &U {
        let inner = unsafe { &*self.inner };
        &inner.metadata
    }

    /// Run the given `predicate` on each node on the snapshot list starting from
    /// the current one all the way up to the head. Returns `Some` as soon as the
    /// predicate returns `Some` and `None` otherwise.
    #[inline]
    pub fn find<'a, F, R>(&'a self, predicate: F) -> Option<R>
    where
        F: Fn(&'a T) -> Option<R>,
    {
        let mut current_ptr = self.inner;

        loop {
            if current_ptr.is_null() {
                break;
            }

            debug_assert!(!current_ptr.is_null());
            let current = unsafe { &*current_ptr };

            if let Some(data) = current.data.load() {
                if let Some(result) = predicate(data) {
                    return Some(result);
                }
            } else {
                break;
            }

            current_ptr = current.next.load(Ordering::Relaxed);
        }

        None
    }
}

impl<T, U> Drop for Snapshot<T, U> {
    fn drop(&mut self) {
        debug_assert!(!self.inner.is_null());
        let inner = unsafe { &*self.inner };
        inner.counter.fetch_sub(1, Ordering::Relaxed);
    }
}

impl<T, U> Drop for SnapshotList<T, U> {
    fn drop(&mut self) {
        let mut tail = self.tail.load(Ordering::Relaxed);

        loop {
            if tail.is_null() {
                break;
            }

            debug_assert!(!tail.is_null());
            let tail_ref = unsafe { &*tail };

            unsafe {
                self.collector
                    .retire(tail, reclaim::boxed::<SnapshotInner<T, U>>);
            }

            // move to the next node.
            tail = tail_ref.next.load(Ordering::Relaxed);
        }
    }
}

#[cfg(tests)]
mod tests {
    // TODO(qti3e): test this data structure. test ideas:
    // 1. `find` method should work in the single-thread mode.
    //      1.1. Should return `None`
}
