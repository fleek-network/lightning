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
    table::{ResolvedTableReference, TableMeta},
};

const INSTANCE_COUNT: AtomicUsize = AtomicUsize::new(0);

pub struct AtomoInner<S: SerdeBackend> {
    /// The unique id of this instance in the entire program.
    pub id: usize,
    /// The mock persistence layer
    pub persistence: Vec<DashMap<Box<[u8]>, Box<[u8]>, fxhash::FxBuildHasher>>,
    /// The tables and dynamic runtime types.
    pub tables: Vec<TableMeta>,
    /// Map each table name to its index.
    pub table_name_to_id: FxHashMap<String, TableId>,
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
            serde: PhantomData,
        }
    }

    /// Return the raw byte representation of a value for a raw key.
    pub fn get_raw(&self, tid: TableId, key: &[u8]) -> Option<Vec<u8>> {
        self.persistence[tid as usize].get(key).map(|v| v.to_vec())
    }

    pub fn get<V>(&self, tid: TableId, key: &[u8]) -> Option<V>
    where
        V: Serialize + DeserializeOwned + Any,
    {
        self.get_raw(tid, &key).map(|v| S::deserialize(&v))
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
}
