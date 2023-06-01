use std::{
    any::{Any, TypeId},
    borrow::Borrow,
    cell::RefCell,
    hash::Hash,
    marker::PhantomData,
    sync::Arc,
};

use fxhash::FxHashSet;
use serde::{de::DeserializeOwned, Serialize};

use crate::{
    batch::{BatchReference, Operation, VerticalBatch},
    db::TableId,
    inner::AtomoInner,
    serder::SerdeBackend,
    snapshot::Snapshot,
};

pub struct TableMeta {
    pub _name: String,
    pub k_id: TypeId,
    pub v_id: TypeId,
}

#[derive(Clone, Copy)]
pub struct ResolvedTableReference<K, V> {
    atomo_id: usize,
    index: TableId,
    kv: PhantomData<(K, V)>,
}

/// The table selector contain multiple tables and is provided to the
/// user at the beginning of a query or an update.
pub struct TableSelector<S: SerdeBackend> {
    /// The [`Atomo`] instance.
    atomo: Arc<AtomoInner<S>>,
    /// The current version of the data.
    snapshot: Snapshot<VerticalBatch>,
    /// A set of already claimed tables.
    selected: RefCell<FxHashSet<TableId>>,
    /// The changes happening here.
    batch: VerticalBatch,
}

pub struct TableRef<
    'selector,
    K: Hash + Eq + Serialize + DeserializeOwned + Any,
    V: Serialize + DeserializeOwned + Any,
    S: SerdeBackend,
> {
    tid: TableId,
    batch: BatchReference,
    selector: &'selector TableSelector<S>,
    kv: PhantomData<(K, V)>,
}

impl TableMeta {
    #[inline(always)]
    pub fn new<K: Any, V: Any>(_name: String) -> Self {
        let k_id = TypeId::of::<K>();
        let v_id = TypeId::of::<V>();
        Self { _name, k_id, v_id }
    }
}

impl<S: SerdeBackend> TableSelector<S> {
    /// Return the table reference for the table with the provided name and K, V type.
    ///
    /// # Panics
    ///
    /// If the information provided are not correct and such a table does not exists.
    pub fn get_table<K, V>(&self, name: impl AsRef<str>) -> TableRef<K, V, S>
    where
        K: Hash + Eq + Serialize + DeserializeOwned + Any,
        V: Serialize + DeserializeOwned + Any,
    {
        self.atomo.resolve::<K, V>(name).get(self)
    }
}

impl<K, V> ResolvedTableReference<K, V> {
    pub(crate) fn new(atomo_id: usize, index: TableId) -> Self {
        ResolvedTableReference {
            atomo_id,
            index,
            kv: PhantomData,
        }
    }

    /// Return the table reference for this table.
    ///
    /// # Panics
    ///
    /// If the table is already claimed.
    pub fn get<'selector, S: SerdeBackend>(
        &self,
        selector: &'selector TableSelector<S>,
    ) -> TableRef<'selector, K, V, S>
    where
        K: Hash + Eq + Serialize + DeserializeOwned + Any,
        V: Serialize + DeserializeOwned + Any,
    {
        assert_eq!(
            self.atomo_id, selector.atomo.id,
            "Table reference of another MtAtomo was used."
        );

        if !selector.selected.borrow_mut().insert(self.index) {
            panic!("Table reference is already claimed.");
        }

        let batch = unsafe { selector.batch.claim(self.index as usize) };

        TableRef {
            tid: self.index,
            batch,
            selector,
            kv: PhantomData,
        }
    }
}

impl<'selector, K, V, S: SerdeBackend> TableRef<'selector, K, V, S>
where
    K: Hash + Eq + Serialize + DeserializeOwned + Any,
    V: Serialize + DeserializeOwned + Any,
{
    // /// Returns an iterator of the keys in this table.
    // pub fn keys<'iter, 'table>(&'table self) -> KeyIterator<'iter, K, V, S>
    // where
    //     'selector: 'iter,
    //     'iter: 'table,
    // {
    //     todo!()
    // }

    pub fn insert(&mut self, key: impl Borrow<K>, value: impl Borrow<V>) {
        let k = S::serialize(key.borrow()).into_boxed_slice();
        let v = S::serialize(value.borrow()).into_boxed_slice();
        self.batch.as_mut().insert(k, Operation::Insert(v));
    }

    pub fn remove(&mut self, key: impl Borrow<K>) {
        let k = S::serialize(key.borrow()).into_boxed_slice();
        self.batch.as_mut().insert(k, Operation::Remove);
    }

    pub fn get(&self, key: impl Borrow<K>) -> Option<V> {
        let k = S::serialize(key.borrow()).into_boxed_slice();

        match self.batch.as_mut().get(&k) {
            Some(Operation::Insert(value)) => return Some(S::deserialize(&value)),
            Some(Operation::Remove) => return None,
            _ => {},
        }

        let index = self.tid as usize;
        if let Some(operation) = self
            .selector
            .snapshot
            .find(|batch| batch.get(index).get(&k))
        {
            match operation {
                Operation::Remove => return None,
                Operation::Insert(value) => return Some(S::deserialize(&value)),
            }
        }

        self.selector.atomo.get::<V>(self.tid, &k)
    }
}
