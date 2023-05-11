use std::{hash::Hash, sync::Arc};

use fxhash::FxHashMap;

use crate::{atomic::OncePtr, atomo::Operation};

pub(crate) struct Snapshot<K, V> {
    entries: OncePtr<FxHashMap<K, Operation<V>>>,
    next: OncePtr<Arc<Snapshot<K, V>>>,
}

impl<K, V> Snapshot<K, V> {
    pub fn new() -> Self {
        Snapshot {
            entries: OncePtr::null(),
            next: OncePtr::null(),
        }
    }

    pub fn with_empty_entries_and_next(next: Arc<Snapshot<K, V>>) -> Self {
        Snapshot {
            entries: OncePtr::new(FxHashMap::default()),
            next: OncePtr::new(next),
        }
    }

    pub fn into_entries(self) -> Option<FxHashMap<K, Operation<V>>> {
        self.entries.into_inner()
    }

    /// # Panics
    ///
    /// If the entries is already set.
    #[inline]
    pub fn set_entries(&self, entries: FxHashMap<K, Operation<V>>) {
        self.entries.store(entries);
    }

    #[inline]
    pub fn set_next(&self, next: Arc<Snapshot<K, V>>) {
        self.next.store(next);
    }
}

impl<K, V> Snapshot<K, V>
where
    K: Hash + Eq,
{
    #[inline]
    pub fn insert(&mut self, key: K, value: V) {
        debug_assert!(!self.entries.is_null());
        unsafe { self.entries.load_mut_unchecked() }.insert(key, Operation::Put(value));
    }

    #[inline]
    pub fn remove(&mut self, key: K) {
        debug_assert!(!self.entries.is_null());
        unsafe { self.entries.load_mut_unchecked() }.insert(key, Operation::Delete);
    }

    #[inline]
    pub fn get(&self, key: &K) -> Option<Option<&V>> {
        let mut current = self;

        loop {
            if let Some(entries) = current.entries.load() {
                match entries.get(key) {
                    Some(Operation::Put(v)) => return Some(Some(v)),
                    Some(Operation::Delete) => return Some(None),
                    None => {}
                }
            }

            if let Some(next) = current.next.load() {
                current = next.as_ref();
            } else {
                break;
            }
        }

        None
    }
}
