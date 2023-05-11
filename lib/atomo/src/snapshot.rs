use std::hash::Hash;

use fxhash::FxHashMap;

use crate::db::Operation;
use crate::gc_list::GcNode;

pub(crate) type Snapshot<K, V> = GcNode<SnapshotData<K, V>>;

pub(crate) struct SnapshotData<K, V>(pub FxHashMap<K, Operation<V>>);

impl<K, V> Default for SnapshotData<K, V> {
    fn default() -> Self {
        Self(FxHashMap::default())
    }
}

impl<K, V> Snapshot<K, V>
where
    K: Hash + Eq,
{
    #[inline]
    pub fn insert(&mut self, key: K, value: V) {
        debug_assert!(!self.value.is_null());
        unsafe { self.value.load_mut_unchecked() }
            .0
            .insert(key, Operation::Put(value));
    }

    #[inline]
    pub fn remove(&mut self, key: K) {
        debug_assert!(!self.value.is_null());
        unsafe { self.value.load_mut_unchecked() }
            .0
            .insert(key, Operation::Delete);
    }

    #[inline]
    pub fn get(&self, key: &K) -> Option<Option<&V>> {
        let mut current = self;

        loop {
            if let Some(entries) = current.value.load() {
                match entries.0.get(key) {
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
