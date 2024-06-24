use std::any::{Any, TypeId};
use std::hash::Hash;
use std::marker::PhantomData;
use std::sync::atomic::AtomicUsize;

use fxhash::FxHashMap;
use serde::de::DeserializeOwned;
use serde::Serialize;

use crate::batch::{Operation, VerticalBatch};
use crate::db::TableId;
use crate::keys::VerticalKeys;
use crate::serder::SerdeBackend;
use crate::snapshot::SnapshotList;
use crate::storage::StorageBackend;
use crate::table::{ResolvedTableReference, TableMeta};

static INSTANCE_COUNT: AtomicUsize = AtomicUsize::new(0);

pub struct AtomoInner<B, S: SerdeBackend> {
    /// The unique id of this instance in the entire program.
    pub id: usize,
    /// The persistence layer
    pub persistence: B,
    /// The tables and dynamic runtime types.
    pub tables: Vec<TableMeta>,
    /// Map each table name to its index.
    pub table_name_to_id: FxHashMap<String, TableId>,
    /// The linked list of the old-snapshots.
    pub snapshot_list: SnapshotList<VerticalBatch, VerticalKeys>,
    serde: PhantomData<S>,
}

impl<S: SerdeBackend> AtomoInner<(), S> {
    pub fn empty() -> Self {
        let id = INSTANCE_COUNT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        AtomoInner {
            id,
            persistence: (),
            tables: Vec::new(),
            table_name_to_id: FxHashMap::default(),
            snapshot_list: SnapshotList::default(),
            serde: PhantomData,
        }
    }

    pub fn swap_persistance<P: StorageBackend>(self, persistence: P) -> AtomoInner<P, S> {
        AtomoInner {
            id: self.id,
            persistence,
            tables: self.tables,
            table_name_to_id: self.table_name_to_id,
            snapshot_list: self.snapshot_list,
            serde: PhantomData,
        }
    }
}

impl<B: StorageBackend, S: SerdeBackend> AtomoInner<B, S> {
    /// Performs a batch of operations on the persistence layer.
    pub fn perform_batch(&self, batch: VerticalBatch) {
        self.persistence.commit(batch);
    }

    /// Given the name of a table as an input string returns a [`ResolvedTableReference`].
    ///
    /// # Panics
    ///
    /// This method panics if:
    ///
    /// 1. The table with the given name does not exists.
    /// 2. The generic types passed for the key-value pair mismatch from the type that was used when
    ///    constructing atomo.
    #[inline]
    pub fn resolve<K, V>(&self, name: impl AsRef<str>) -> ResolvedTableReference<K, V>
    where
        K: Hash + Eq + Serialize + DeserializeOwned + Any,
        V: Serialize + DeserializeOwned + Any,
    {
        let name = name.as_ref();
        let index = *self
            .table_name_to_id
            .get(name)
            .unwrap_or_else(|| panic!("Table {name} not found."));

        let info = &self.tables[index as usize];
        let k_id = TypeId::of::<K>();
        let v_id = TypeId::of::<V>();
        let k_str = std::any::type_name::<K>();
        let v_str = std::any::type_name::<V>();

        assert_eq!(
            info.k_id, k_id,
            "Could not resolve table '{name}' with key type '{k_str}'."
        );

        assert_eq!(
            info.v_id, v_id,
            "Could not resolve table '{name}' with value type '{v_str}'."
        );

        ResolvedTableReference::<K, V>::new(self.id, index)
    }

    /// Given a vertical batch (which we intend to commit) compute the inverse of the batch. The
    /// inverse of a batch is another batch that when executed reverts the changes.
    #[inline]
    pub fn compute_inverse(&self, batch: &VerticalBatch) -> VerticalBatch {
        let num_tables = self.tables.len() as u8;
        let mut inverse = VerticalBatch::new(num_tables);

        for t in 0..num_tables {
            let table = inverse.get_mut(t);
            let batch_table = batch.get(t);
            table.reserve(batch_table.len());

            for (key, op) in batch_table.iter() {
                match (op, self.get_raw(t as TableId, key)) {
                    (Operation::Remove, Some(old_value)) => {
                        table.insert(key.clone(), Operation::Insert(old_value.into_boxed_slice()));
                    },
                    (Operation::Insert(_new_value), None) => {
                        table.insert(key.clone(), Operation::Remove);
                    },
                    (Operation::Insert(new_value), Some(old_value))
                        if new_value[..] != old_value[..] =>
                    {
                        table.insert(key.clone(), Operation::Insert(old_value.into_boxed_slice()));
                    },
                    // Change has not happened.
                    (Operation::Remove, None) => {},
                    (Operation::Insert(_new_value), Some(_old_value)) => {}, /* we matched the !=
                                                                              * before. */
                }
            }

            // if the table has more than half empty slots shrink it.
            if table.capacity() >= table.len() * 2 {
                table.shrink_to_fit();
            }
        }

        inverse
    }
}

impl<B: StorageBackend, S: SerdeBackend> AtomoInner<B, S> {
    /// Return the raw byte representation of a value for a raw key.
    pub fn get_raw(&self, tid: TableId, key: &[u8]) -> Option<Vec<u8>> {
        self.persistence.get(tid, key)
    }

    /// Returns the deserialized value associated with a key.
    pub fn get<V>(&self, tid: TableId, key: &[u8]) -> Option<V>
    where
        V: Serialize + DeserializeOwned + Any,
    {
        self.get_raw(tid, key).map(|v| S::deserialize(&v))
    }

    /// Returns true if the key exists.
    pub fn contains_key(&self, tid: TableId, key: &[u8]) -> bool {
        self.persistence.contains(tid, key)
    }
}

#[cfg(test)]
mod tests {
    use crate::batch::{Operation, VerticalBatch};
    use crate::storage::InMemoryStorage;
    use crate::{AtomoBuilder, BincodeSerde};

    #[test]
    fn resolve_valid_should_work() {
        let inner = AtomoBuilder::<InMemoryStorage, BincodeSerde>::default()
            .with_table::<String, usize>("TABLE")
            .build_inner()
            .unwrap();

        inner.resolve::<String, usize>("TABLE");
    }

    #[test]
    #[should_panic]
    fn resolve_key_type_mismatch_should_panic() {
        let inner = AtomoBuilder::<InMemoryStorage, BincodeSerde>::default()
            .with_table::<String, usize>("TABLE")
            .build_inner()
            .unwrap();

        inner.resolve::<Vec<u8>, usize>("TABLE");
    }

    #[test]
    #[should_panic]
    fn resolve_value_type_mismatch_should_panic() {
        let inner = AtomoBuilder::<InMemoryStorage, BincodeSerde>::default()
            .with_table::<String, usize>("TABLE")
            .build_inner()
            .unwrap();

        inner.resolve::<String, u8>("TABLE");
    }

    #[test]
    #[should_panic]
    fn resolve_undefined_table_should_panic() {
        let inner = AtomoBuilder::<InMemoryStorage, BincodeSerde>::default()
            .with_table::<String, usize>("TABLE")
            .build_inner()
            .unwrap();

        inner.resolve::<String, usize>("TABLE-X");
    }

    #[test]
    fn perform_batch() {
        let inner = AtomoBuilder::<InMemoryStorage, BincodeSerde>::default()
            .with_table::<Vec<u8>, usize>("TABLE")
            .build_inner()
            .unwrap();

        let mut batch = VerticalBatch::new(1);
        let map = batch.get_mut(0);
        map.insert(
            vec![0].into_boxed_slice(),
            Operation::Insert(vec![1].into_boxed_slice()),
        );
        map.insert(
            vec![1].into_boxed_slice(),
            Operation::Insert(vec![2].into_boxed_slice()),
        );
        map.insert(
            vec![2].into_boxed_slice(),
            Operation::Insert(vec![3].into_boxed_slice()),
        );
        inner.perform_batch(batch);

        assert_eq!(inner.get_raw(0, &[0]), Some(vec![1]));
        assert_eq!(inner.get_raw(0, &[1]), Some(vec![2]));
        assert_eq!(inner.get_raw(0, &[2]), Some(vec![3]));
        assert_eq!(inner.get_raw(0, &[3]), None);
        assert!(inner.contains_key(0, &[0]));
        assert!(inner.contains_key(0, &[1]));
        assert!(inner.contains_key(0, &[2]));
        assert!(!inner.contains_key(0, &[3]));

        let mut batch = VerticalBatch::new(1);
        let map = batch.get_mut(0);
        // update
        map.insert(
            vec![0].into_boxed_slice(),
            Operation::Insert(vec![4].into_boxed_slice()),
        );
        map.insert(vec![1].into_boxed_slice(), Operation::Remove);
        // default insert
        map.insert(
            vec![3].into_boxed_slice(),
            Operation::Insert(vec![5].into_boxed_slice()),
        );
        inner.perform_batch(batch);

        assert_eq!(inner.get_raw(0, &[0]), Some(vec![4]));
        assert_eq!(inner.get_raw(0, &[1]), None);
        assert_eq!(inner.get_raw(0, &[2]), Some(vec![3]));
        assert_eq!(inner.get_raw(0, &[3]), Some(vec![5]));
        assert!(inner.contains_key(0, &[0]));
        assert!(!inner.contains_key(0, &[1]));
        assert!(inner.contains_key(0, &[2]));
        assert!(inner.contains_key(0, &[3]));
    }

    #[test]
    fn compute_inverse() {
        let inner = AtomoBuilder::<InMemoryStorage, BincodeSerde>::default()
            .with_table::<Vec<u8>, usize>("TABLE")
            .build_inner()
            .unwrap();

        let mut batch = VerticalBatch::new(1);
        let map = batch.get_mut(0);
        map.insert(
            vec![0].into_boxed_slice(),
            Operation::Insert(vec![1].into_boxed_slice()),
        );
        map.insert(
            vec![1].into_boxed_slice(),
            Operation::Insert(vec![2].into_boxed_slice()),
        );
        map.insert(
            vec![2].into_boxed_slice(),
            Operation::Insert(vec![3].into_boxed_slice()),
        );
        inner.perform_batch(batch);

        let mut batch = VerticalBatch::new(1);
        let map = batch.get_mut(0);
        // update key
        map.insert(
            vec![0].into_boxed_slice(),
            Operation::Insert(vec![4].into_boxed_slice()),
        );
        // remove key
        map.insert(vec![1].into_boxed_slice(), Operation::Remove);
        // for key=2 preserve it.
        // and insert a default key.
        map.insert(
            vec![3].into_boxed_slice(),
            Operation::Insert(vec![5].into_boxed_slice()),
        );
        let inverse = inner.compute_inverse(&batch);
        inner.perform_batch(batch);

        // Check if the batch was actually performed.
        assert_eq!(inner.get_raw(0, &[0]), Some(vec![4]));
        assert_eq!(inner.get_raw(0, &[1]), None);
        assert_eq!(inner.get_raw(0, &[2]), Some(vec![3]));
        assert_eq!(inner.get_raw(0, &[3]), Some(vec![5]));

        // now revert should put us back to where we started.
        inner.perform_batch(inverse);
        assert_eq!(inner.get_raw(0, &[0]), Some(vec![1]));
        assert_eq!(inner.get_raw(0, &[1]), Some(vec![2]));
        assert_eq!(inner.get_raw(0, &[2]), Some(vec![3]));
        assert_eq!(inner.get_raw(0, &[3]), None);
    }

    #[test]
    fn compute_inverse_on_empty_db() {
        let inner = AtomoBuilder::<InMemoryStorage, BincodeSerde>::default()
            .with_table::<Vec<u8>, usize>("TABLE")
            .build_inner()
            .unwrap();

        let mut batch = VerticalBatch::new(1);
        let map = batch.get_mut(0);
        map.insert(
            vec![0].into_boxed_slice(),
            Operation::Insert(vec![1].into_boxed_slice()),
        );
        map.insert(
            vec![1].into_boxed_slice(),
            Operation::Insert(vec![2].into_boxed_slice()),
        );
        map.insert(
            vec![2].into_boxed_slice(),
            Operation::Insert(vec![3].into_boxed_slice()),
        );
        let inverse = inner.compute_inverse(&batch);
        inner.perform_batch(batch);
        inner.perform_batch(inverse);

        assert_eq!(inner.get_raw(0, &[0]), None);
        assert_eq!(inner.get_raw(0, &[1]), None);
        assert_eq!(inner.get_raw(0, &[2]), None);
        assert_eq!(inner.get_raw(0, &[3]), None);
    }
}
