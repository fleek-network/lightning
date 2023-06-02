use std::{
    any::{Any, TypeId},
    hash::Hash,
    marker::PhantomData,
    sync::atomic::AtomicUsize,
};

use dashmap::DashMap;
use fxhash::FxHashMap;
use serde::{de::DeserializeOwned, Serialize};

use crate::{
    batch::{Operation, VerticalBatch},
    db::TableId,
    serder::SerdeBackend,
    snapshot::SnapshotList,
    table::{ResolvedTableReference, TableMeta},
};

static INSTANCE_COUNT: AtomicUsize = AtomicUsize::new(0);

type MockPersistance = DashMap<Box<[u8]>, Box<[u8]>, fxhash::FxBuildHasher>;

pub struct AtomoInner<S: SerdeBackend> {
    /// The unique id of this instance in the entire program.
    pub id: usize,
    /// The mock persistence layer
    pub persistence: Vec<MockPersistance>,
    /// The tables and dynamic runtime types.
    pub tables: Vec<TableMeta>,
    /// Map each table name to its index.
    pub table_name_to_id: FxHashMap<String, TableId>,
    /// The linked list of the old-snapshots.
    pub snapshot_list: SnapshotList<VerticalBatch>,
    serde: PhantomData<S>,
}

impl<S: SerdeBackend> AtomoInner<S> {
    pub fn empty() -> Self {
        let id = INSTANCE_COUNT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        AtomoInner {
            id,
            persistence: Vec::new(),
            tables: Vec::new(),
            table_name_to_id: FxHashMap::default(),
            snapshot_list: SnapshotList::new(),
            serde: PhantomData,
        }
    }

    /// Performs a batch of operations on the persistence layer.
    pub fn perform_batch(&self, batch: VerticalBatch) {
        for (table, batch) in self.persistence.iter().zip(batch.into_raw().into_iter()) {
            for (key, operation) in batch.into_iter() {
                match operation {
                    Operation::Remove => {
                        table.remove(&key);
                    },
                    Operation::Insert(value) => {
                        table.insert(key, value);
                    },
                }
            }
        }
    }

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
            "Could not resolve table '{name}' with key type '{v_str}'."
        );

        ResolvedTableReference::<K, V>::new(self.id, index)
    }

    #[inline]
    pub fn compute_inverse(&self, batch: &VerticalBatch) -> VerticalBatch {
        let num_tables = self.tables.len();
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

impl<S: SerdeBackend> AtomoInner<S> {
    /// Return the raw byte representation of a value for a raw key.
    pub fn get_raw(&self, tid: TableId, key: &[u8]) -> Option<Vec<u8>> {
        self.persistence[tid as usize].get(key).map(|v| v.to_vec())
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
        self.persistence[tid as usize].contains_key(key)
    }
}
