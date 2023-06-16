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
    keys::VerticalKeys,
    serder::SerdeBackend,
    snapshot::Snapshot,
    KeyIterator,
};

pub struct TableMeta {
    pub _name: String,
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
pub struct TableSelector<S: SerdeBackend> {
    /// The [`Atomo`] instance.
    atomo: Arc<AtomoInner<S>>,
    /// The current version of the data.
    snapshot: Snapshot<VerticalBatch, VerticalKeys>,
    /// A set of already claimed tables.
    // TODO(qti3e): Replace this with a UnsafeCell or a `SingleThreadedBoolVec`.
    selected: RefCell<FxHashSet<TableId>>,
    /// The *hot* changes happening here in this run.
    batch: VerticalBatch,
}

/// A reference to a table inside an execution context (i.e [`TableSelector`]). A table reference
/// can be used as a reference to a table to operate on it.
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

// When a table ref is dropped make it available to be claimed again.
impl<'selector, K, V, S: SerdeBackend> Drop for TableRef<'selector, K, V, S>
where
    K: Hash + Eq + Serialize + DeserializeOwned + Any,
    V: Serialize + DeserializeOwned + Any,
{
    fn drop(&mut self) {
        self.selector.selected.borrow_mut().remove(&self.tid);
    }
}

impl<S: SerdeBackend> TableSelector<S> {
    /// Create a new table selector for the head of an Atomo instance.
    #[inline]
    pub fn new(atomo: Arc<AtomoInner<S>>) -> Self {
        let num_tables = atomo.tables.len();
        let batch = VerticalBatch::new(num_tables);
        let snapshot = atomo.snapshot_list.current();

        Self {
            atomo,
            snapshot,
            selected: RefCell::new(FxHashSet::default()),
            batch,
        }
    }

    #[inline]
    pub(crate) fn into_batch(self) -> VerticalBatch {
        self.batch
    }

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

    /// Returns the table reference for this table.
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

impl<'selector, K, V, S: SerdeBackend> TableRef<'selector, K, V, S>
where
    K: Hash + Eq + Serialize + DeserializeOwned + Any,
    V: Serialize + DeserializeOwned + Any,
{
    /// Insert a new `key` and `value` pair into the table.
    pub fn insert(&mut self, key: impl Borrow<K>, value: impl Borrow<V>) {
        let k = S::serialize(key.borrow()).into_boxed_slice();
        let v = S::serialize(value.borrow()).into_boxed_slice();
        self.batch.as_mut().insert(k, Operation::Insert(v));
    }

    /// Remove the given key from the table.
    pub fn remove(&mut self, key: impl Borrow<K>) {
        let k = S::serialize(key.borrow()).into_boxed_slice();
        self.batch.as_mut().insert(k, Operation::Remove);
    }

    /// Returns the value associated with the provided key. If the key doesn't exits in the table
    /// [`None`] is returned.
    pub fn get(&self, key: impl Borrow<K>) -> Option<V> {
        let k = S::serialize(key.borrow()).into_boxed_slice();

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

        self.selector.atomo.get::<V>(self.tid, &k)
    }

    /// Returns `true` if the key exists in the table.
    pub fn contains_key(&self, key: impl Borrow<K>) -> bool {
        let k = S::serialize(key.borrow()).into_boxed_slice();

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
    pub fn keys<'iter, 'table>(&'table self) -> KeyIterator<'iter, K, S>
    where
        'selector: 'iter,
        'iter: 'table,
    {
        todo!()
    }
}
