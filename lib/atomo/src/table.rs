use std::any::{Any, TypeId};
use std::borrow::Borrow;
use std::cell::RefCell;
use std::hash::Hash;
use std::marker::PhantomData;
use std::sync::Arc;

use fxhash::FxHashSet;
use serde::de::DeserializeOwned;
use serde::Serialize;

use crate::batch::{BatchReference, Operation, VerticalBatch};
use crate::db::TableId;
use crate::inner::AtomoInner;
use crate::keys::VerticalKeys;
use crate::serder::SerdeBackend;
use crate::snapshot::Snapshot;
use crate::{KeyIterator, StorageBackend};

#[derive(Clone)]
pub struct TableMeta {
    pub name: String,
    pub k_id: TypeId,
    pub v_id: TypeId,
}

/// A resolved table reference can be used to cache the lookup of a table by its string name
/// and the type validations and can be used to speed up the [`TableSelector::get_table`] function.
///
/// This can be achieved by pre-computing a bunch of [`ResolvedTableReference`]s before invoking
/// the `run` function and use the resolved references to get a [`TableRef`].
///
/// [`ResolvedTableReference`]s of one `Atomo` instance can not be used for another instance, and
/// that will cause a panic.
#[derive(Clone, Copy)]
pub struct ResolvedTableReference<K, V> {
    /// The ID for the `Atomo` instance.
    atomo_id: usize,
    /// The index of the table.
    index: TableId,
    kv: PhantomData<(K, V)>,
}

/// The table selector contain multiple tables and is provided to the user at the beginning of a
/// query or an update. You can think about this as the execution context.
///
/// This is given as a parameter to the callback which you pass to the [`crate::Atomo::run`]
/// function.
///
/// Once you have an instance of a table selector, you can get a reference to a specific table.
///
/// And at that point you can use that table reference instance to operate on a table or retrieve
/// values from it.
pub struct TableSelector<B: StorageBackend, S: SerdeBackend> {
    /// The [`Atomo`] instance.
    atomo: Arc<AtomoInner<B, S>>,
    /// The current version of the data.
    snapshot: Snapshot<VerticalBatch, VerticalKeys>,
    /// A set of already claimed tables.
    // TODO(qti3e): Replace this with a UnsafeCell or a `SingleThreadedBoolVec`.
    selected: RefCell<FxHashSet<TableId>>,
    /// The *hot* changes happening here in this run.
    batch: VerticalBatch,
    /// The new version of the keys.
    keys: RefCell<VerticalKeys>,
}

/// A reference to a table inside an execution context (i.e [`TableSelector`]). A table reference
/// can be used as a reference to a table to operate on it.
pub struct TableRef<
    'selector,
    K: Hash + Eq + Serialize + DeserializeOwned + Any,
    V: Serialize + DeserializeOwned + Any,
    B: StorageBackend,
    S: SerdeBackend,
> {
    tid: TableId,
    batch: BatchReference,
    selector: &'selector TableSelector<B, S>,
    kv: PhantomData<(K, V)>,
}

impl TableMeta {
    #[inline(always)]
    pub fn new<K: Any, V: Any>(name: String) -> Self {
        let k_id = TypeId::of::<K>();
        let v_id = TypeId::of::<V>();
        Self { name, k_id, v_id }
    }
}

// When a table ref is dropped make it available to be claimed again.
impl<'selector, K, V, B: StorageBackend, S: SerdeBackend> Drop for TableRef<'selector, K, V, B, S>
where
    K: Hash + Eq + Serialize + DeserializeOwned + Any,
    V: Serialize + DeserializeOwned + Any,
{
    fn drop(&mut self) {
        self.selector.selected.borrow_mut().remove(&self.tid);
    }
}

impl<B: StorageBackend, S: SerdeBackend> TableSelector<B, S> {
    /// Create a new table selector for the head of an Atomo instance.
    #[inline]
    pub fn new(atomo: Arc<AtomoInner<B, S>>) -> Self {
        let num_tables = atomo.tables.len();
        let batch = VerticalBatch::new(num_tables);
        let snapshot = atomo.snapshot_list.current();
        let keys = snapshot.get_metadata().clone();

        Self {
            atomo,
            snapshot,
            selected: RefCell::new(FxHashSet::default()),
            batch,
            keys: RefCell::new(keys),
        }
    }

    #[inline]
    pub(crate) fn into_raw(self) -> (VerticalBatch, VerticalKeys) {
        (self.batch, self.keys.into_inner())
    }

    #[inline]
    pub fn batch(&self) -> VerticalBatch {
        self.batch.clone()
    }

    #[inline]
    pub fn tables(&self) -> Vec<TableMeta> {
        self.atomo.tables.clone()
    }

    /// Return the table reference for the table with the provided name and K, V type.
    ///
    /// # Panics
    ///
    /// If the information provided are not correct and such a table does not exists.
    pub fn get_table<K, V>(&self, name: impl AsRef<str>) -> TableRef<K, V, B, S>
    where
        K: Hash + Eq + Serialize + DeserializeOwned + Any,
        V: Serialize + DeserializeOwned + Any,
    {
        self.atomo.resolve::<K, V>(name).get(self)
    }

    /// Get the raw, serialized bytes value of a given table and key. The key is also expected to be
    /// serialized bytes.
    /// Returns [`None`] if the table or the key is not found.
    pub fn get_raw_value(&self, table: impl AsRef<str>, key: &[u8]) -> Option<Vec<u8>> {
        self.atomo
            .table_name_to_id
            .get(table.as_ref())
            .and_then(|tid| self.atomo.get_raw(*tid, key))
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

    /// Returns the table reference for this table.
    ///
    /// # Panics
    ///
    /// If the table is already claimed.
    pub fn get<'selector, B: StorageBackend, S: SerdeBackend>(
        &self,
        selector: &'selector TableSelector<B, S>,
    ) -> TableRef<'selector, K, V, B, S>
    where
        K: Hash + Eq + Serialize + DeserializeOwned + Any,
        V: Serialize + DeserializeOwned + Any,
    {
        assert_eq!(
            self.atomo_id, selector.atomo.id,
            "Table reference of another Atomo instance was used."
        );

        if !selector.selected.borrow_mut().insert(self.index) {
            panic!("Table reference is already claimed.");
        }

        // Safety:
        //
        // 1. Using the previous assertion, we ensure that the table is not yet
        // claimed, and hence only one mutable reference to a table exists.
        //
        // 2. The returned reference has lifetime `'selector` and batch is owned by
        // the selector, and since a selector never replaces its batch, we ensure that
        //      1. The returned reference never outlives the 'batch'.
        //      2. The batch never go out of scope before returned reference is dropped.
        let batch = unsafe { selector.batch.claim(self.index as usize) };

        TableRef {
            tid: self.index,
            batch,
            selector,
            kv: PhantomData,
        }
    }
}

impl<'selector, K, V, B: StorageBackend, S: SerdeBackend> TableRef<'selector, K, V, B, S>
where
    K: Hash + Eq + Serialize + DeserializeOwned + Any,
    V: Serialize + DeserializeOwned + Any,
{
    /// Insert a new `key` and `value` pair into the table.
    pub fn insert(&mut self, key: impl Borrow<K>, value: impl Borrow<V>) {
        let k = S::serialize(key.borrow()).into_boxed_slice();
        let v = S::serialize(value.borrow()).into_boxed_slice();
        self.selector
            .keys
            .borrow_mut()
            .update(self.tid, |collection| {
                collection.insert(k.clone());
            });
        self.batch.as_mut().insert(k, Operation::Insert(v));
    }

    /// Remove the given key from the table.
    pub fn remove(&mut self, key: impl Borrow<K>) {
        let k = S::serialize(key.borrow()).into_boxed_slice();
        self.selector
            .keys
            .borrow_mut()
            .update(self.tid, |collection| {
                collection.remove(&k);
            });
        self.batch.as_mut().insert(k, Operation::Remove);
    }

    /// Returns the value associated with the provided key. If the key doesn't exits in the table
    /// [`None`] is returned.
    pub fn get(&self, key: impl Borrow<K>) -> Option<V> {
        let k = S::serialize(key.borrow()).into_boxed_slice();
        // We get the underlying value before checking snapshots to fix a race condition where a
        // value is updated after checking the snapshot and before we grab the data
        // todo: optimize this
        let tmp = self.selector.atomo.get::<V>(self.tid, &k);
        match self.batch.get(&k) {
            Some(Operation::Insert(value)) => return Some(S::deserialize(value)),
            Some(Operation::Remove) => return None,
            _ => {},
        }

        let index = self.tid as usize;
        if let Some(operation) = self
            .selector
            .snapshot
            .find(|batch| batch.get(index).get(&k))
        {
            return match operation {
                Operation::Remove => None,
                Operation::Insert(value) => Some(S::deserialize(value)),
            };
        }

        tmp
    }

    /// Returns `true` if the key exists in the table.
    pub fn contains_key(&self, key: impl Borrow<K>) -> bool {
        let k = S::serialize(key.borrow()).into_boxed_slice();

        {
            let keys_ref = self.selector.keys.borrow();
            if let Some(im) = keys_ref.get(self.tid) {
                return im.contains(&k);
            }
        }

        match self.batch.get(&k) {
            Some(Operation::Insert(_)) => return true,
            Some(Operation::Remove) => return false,
            _ => {},
        }

        let index = self.tid as usize;
        if let Some(op) = self
            .selector
            .snapshot
            .find(|batch| batch.get(index).get(&k))
        {
            return match op {
                Operation::Insert(_) => true,
                Operation::Remove => false,
            };
        }

        self.selector.atomo.contains_key(self.tid, &k)
    }

    /// Returns an iterator of the keys in this table.
    ///
    /// # Panics
    ///
    /// If the current table is not opened with iterator support when opening the
    /// Atomo instance. See the documentation for [`crate::AtomoBuilder::enable_iter`]
    /// for more information.
    pub fn keys(&self) -> KeyIterator<K> {
        let keys = self
            .selector
            .keys
            .borrow()
            .get(self.tid)
            .clone()
            .expect("Iterator functionality is not enabled for the table.");

        KeyIterator::new(keys)
    }
}
