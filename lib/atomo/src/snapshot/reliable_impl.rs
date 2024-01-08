use std::sync::{Arc, RwLock};

use once_ptr::OncePtr;

pub struct SnapshotList<T, U> {
    head: RwLock<Snapshot<T, U>>,
}

/// The snapshot object.
pub struct Snapshot<T, U>(Arc<SnapshotInner<T, U>>);

struct SnapshotInner<T, U> {
    next: OncePtr<Snapshot<T, U>>,
    data: OncePtr<T>,
    metadata: U,
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
        let snapshot = Snapshot(Arc::new(SnapshotInner {
            next: OncePtr::null(),
            data: OncePtr::null(),
            metadata,
        }));

        Self {
            head: RwLock::new(snapshot),
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
        let next_head = Snapshot(Arc::new(SnapshotInner {
            next: OncePtr::null(),
            data: OncePtr::null(),
            metadata,
        }));

        let current = self.current();
        current.0.data.store(data);
        current.0.next.store(next_head.clone());

        transition();

        let mut guard = self.head.write().expect("Could not acquire the lock");
        *guard = next_head;
    }

    /// Returns the current snapshot for the data.
    pub fn current(&self) -> Snapshot<T, U> {
        let guard = self.head.read().expect("Could not acquire read lock");
        guard.clone()
    }

    /// Returns a mutable reference to the head's meta data.
    pub fn get_metadata_mut(&mut self) -> &mut U {
        let current = self.current();
        let metadata_mut_ref = ((&current.0.metadata) as *const U) as *mut U;
        // TODO: (parsa) ensure this can be safely ignored
        #[allow(invalid_reference_casting)]
        unsafe {
            &mut *metadata_mut_ref
        }
    }
}

impl<T, U> Snapshot<T, U> {
    /// Returns the meta data associated with the current snapshot.
    pub fn get_metadata(&self) -> &U {
        &self.0.metadata
    }

    /// Run the given `predicate` on each node on the snapshot list starting from
    /// the current one all the way up to the head. Returns `Some` as soon as the
    /// predicate returns `Some` and `None` otherwise.
    #[inline]
    pub fn find<'a, F, R>(&'a self, predicate: F) -> Option<R>
    where
        F: Fn(&'a T) -> Option<R>,
    {
        let mut current = self;

        loop {
            let data = &current.0.data;

            if let Some(data) = data.load() {
                if let Some(result) = predicate(data) {
                    return Some(result);
                }
            } else {
                break;
            }

            if let Some(next) = current.0.next.load() {
                current = next;
            } else {
                break;
            }
        }

        None
    }
}

impl<T, U> Clone for Snapshot<T, U> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}
