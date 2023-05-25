use std::{
    any::{Any, TypeId},
    cell::RefCell,
    hash::Hash,
    marker::PhantomData,
    sync::{atomic::AtomicUsize, Arc},
};

use dashmap::DashMap;
use fxhash::{FxHashMap, FxHashSet};
use serde::{de::DeserializeOwned, Serialize};

use crate::{
    db::{Batch, Operation},
    gc_list::{GcList, GcNode},
    snapshot::SnapshotData,
    DefaultSerdeBackend, SerdeBackend, Shared,
};

const INSTANCE_COUNT: AtomicUsize = AtomicUsize::new(0);

type TableId = u8;

/// The query permission on an [`MtAtomo`] only allows non-mutating changes.
pub struct QueryPerm;

/// The update permission on an [`MtAtomo`] which allows mutating the data.
pub struct UpdatePerm;

struct TableMeta {
    _name: String,
    k_id: TypeId,
    v_id: TypeId,
}

impl TableMeta {
    #[inline(always)]
    pub fn new<K: Any, V: Any>(_name: String) -> Self {
        let k_id = TypeId::of::<K>();
        let v_id = TypeId::of::<V>();
        Self { _name, k_id, v_id }
    }
}

pub struct MtAtomoBuilder<S: SerdeBackend = DefaultSerdeBackend> {
    atomo: MtAtomoInner<S>,
}

impl<S: SerdeBackend> MtAtomoBuilder<S> {
    #[must_use = "Creating a builder does not perform anything."]
    pub fn new() -> Self {
        Self {
            atomo: MtAtomoInner::empty(),
        }
    }

    /// Open a new table with the given name and key-value type.
    ///
    /// # Panics
    ///
    /// If another table with the given name is already created.
    #[must_use = "Builder is incomplete."]
    #[inline(always)]
    pub fn with_table<K, V>(mut self, name: impl ToString) -> Self
    where
        K: Hash + Eq + Serialize + DeserializeOwned + Any,
        V: Serialize + DeserializeOwned + Any,
    {
        let name = name.to_string();
        self.with_table_internal_non_generic_part(name.clone());
        self.atomo.tables.push(TableMeta::new::<K, V>(name));
        self
    }

    /// Performs the common operation for opening a table that is not depended
    /// on generic types to produce smaller code when `with_table` is inlined.
    fn with_table_internal_non_generic_part(&mut self, name: String) {
        let index = self.atomo.tables.len();

        if index > (TableId::MAX as usize) {
            panic!("Table ID overflow.");
        }

        if self
            .atomo
            .table_name_to_id
            .insert(name.clone(), index as TableId)
            .is_some()
        {
            panic!("Table {name} is already defined.");
        }

        self.atomo.persistence.push(DashMap::default());
    }

    #[must_use = "Creating a MtAtomo without using it is probably a mistake."]
    pub fn build(self) -> MtAtomo<UpdatePerm, S> {
        MtAtomo::new(Arc::new(self.atomo))
    }
}

pub struct MtAtomo<O, S: SerdeBackend = DefaultSerdeBackend> {
    inner: Arc<MtAtomoInner<S>>,
    ownership: PhantomData<O>,
}

// only implement the clone for the query permission.
impl<S: SerdeBackend> Clone for MtAtomo<QueryPerm, S> {
    fn clone(&self) -> Self {
        Self::new(self.inner.clone())
    }
}

impl<O, S: SerdeBackend> MtAtomo<O, S> {
    #[inline]
    fn new(inner: Arc<MtAtomoInner<S>>) -> Self {
        Self {
            inner,
            ownership: PhantomData,
        }
    }

    /// Returns a query end for this table.
    pub fn query(&self) -> MtAtomo<QueryPerm, S> {
        MtAtomo::new(self.inner.clone())
    }

    /// Resolve a table with the given name and key-value types.
    ///
    /// # Panics
    ///
    /// 1. If the table with the provided name does not exists.
    /// 2. The `K` is provided here is not the same type that was used when opening the table.
    /// 3. The `V` is provided here is not the same type that was used when opening the table.
    pub fn resolve<K, V>(&self, name: impl AsRef<str>) -> ResolvedTableReference<K, V>
    where
        K: Hash + Eq + Serialize + DeserializeOwned + Any,
        V: Serialize + DeserializeOwned + Any,
    {
        self.inner.resolve::<K, V>(name)
    }
}

impl<S: SerdeBackend> MtAtomo<QueryPerm, S> {
    pub fn run<F, R>(&self, query: F) -> R
    where
        F: Fn(&mut TableSelector<S>) -> R,
    {
        let mut selector = TableSelector::new(self.inner.clone(), None);
        query(&mut selector)
    }
}

impl<S: SerdeBackend> MtAtomo<UpdatePerm, S> {
    pub fn run<F, R>(&mut self, mutation: F) -> R
    where
        F: Fn(&mut TableSelector<S>) -> R,
    {
        let mut selector = TableSelector::new(
            self.inner.clone(),
            Some(InverseAndBatchList::new(self.inner.tables.len())),
        );

        let result = mutation(&mut selector);

        let mut inv_batch_list = selector.inv_batch_list.unwrap();

        let inv = inv_batch_list.take_inv();

        let next = unsafe { self.inner.head.push(inv) };

        let batches = inv_batch_list.take_batch();
        for (tid, batch) in batches.into_iter().enumerate() {
            self.inner.perform_batch(tid as TableId, batch);
        }

        unsafe {
            self.inner.head.set_head(next);
        }

        result
    }
}

struct MtAtomoInner<S: SerdeBackend> {
    /// The unique id of this instance in the entire program.
    id: usize,
    /// The mock persistence layer
    persistence: Vec<DashMap<Box<[u8]>, Box<[u8]>, fxhash::FxBuildHasher>>,
    /// The head of the current version.
    head: GcList<MtSnapshotData>,
    /// The tables and dynamic runtime types.
    tables: Vec<TableMeta>,
    /// Map each table name to its index.
    table_name_to_id: FxHashMap<String, TableId>,
    serde: PhantomData<S>,
}

impl<S: SerdeBackend> MtAtomoInner<S> {
    fn empty() -> Self {
        let id = INSTANCE_COUNT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        MtAtomoInner {
            id,
            persistence: Vec::new(),
            head: GcList::new(),
            tables: Vec::new(),
            table_name_to_id: FxHashMap::default(),
            serde: PhantomData,
        }
    }

    /// Return the raw byte representation of a value for a raw key.
    fn get_raw(&self, tid: TableId, key: &[u8]) -> Option<Vec<u8>> {
        self.persistence[tid as usize].get(key).map(|v| v.to_vec())
    }

    fn get<K, V>(&self, tid: TableId, key: &K) -> Option<V>
    where
        K: Hash + Eq + Serialize + DeserializeOwned + Any,
        V: Serialize + DeserializeOwned + Any,
    {
        let key_serialized = S::serialize(key);
        self.get_raw(tid, &key_serialized)
            .map(|v| S::deserialize(&v))
    }

    /// Performs a batch of operations on the persistence layer.
    fn perform_batch(&self, tid: TableId, batch: Batch) {
        let table = &self.persistence[tid as usize];
        for (k, op) in batch {
            match op {
                Some(value) => {
                    table.insert(k, value);
                },
                None => {
                    table.remove(&k);
                },
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

/// The table selector contain multiple tables and is provided to the
/// user at the beginning of a query or an update.
pub struct TableSelector<S: SerdeBackend> {
    /// The [`Atomo`] instance.
    atomo: Arc<MtAtomoInner<S>>,
    /// The current version of the data.
    snapshot: Arc<GcNode<MtSnapshotData>>,
    /// A set of already claimed tables.
    selected: RefCell<FxHashSet<TableId>>,
    /// If we want to collect the inverse and batch we should provide
    /// a non-empty value for this.
    inv_batch_list: Option<InverseAndBatchList>,
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

struct InverseAndBatchList {
    /// The inverses that we should put to the previous version,
    /// The value for each table is inserted when drop happens.
    inverse: Vec<RefCell<Option<Box<dyn Any>>>>,
    /// The batches for each table, updated when the [`TableRef`] is dropped.
    batches: Vec<RefCell<Batch>>,
}

impl<S: SerdeBackend> TableSelector<S> {
    #[inline]
    fn new(atomo: Arc<MtAtomoInner<S>>, inv_batch_list: Option<InverseAndBatchList>) -> Self {
        let snapshot = atomo.head.current();

        TableSelector {
            atomo,
            snapshot,
            selected: RefCell::new(FxHashSet::default()),
            inv_batch_list,
        }
    }
}

impl InverseAndBatchList {
    #[inline]
    pub fn new(count: usize) -> Self {
        let mut inverse = Vec::with_capacity(count);
        let mut batches = Vec::with_capacity(count);

        for _ in 0..count {
            inverse.push(RefCell::new(None));
            batches.push(RefCell::new(Vec::new()));
        }

        Self { inverse, batches }
    }

    pub fn take_inv(&mut self) -> MtSnapshotData {
        let mut data = MtSnapshotData {
            tables: Vec::with_capacity(self.inverse.len()),
        };

        for inv in &mut self.inverse {
            data.tables.push(inv.get_mut().take());
        }

        data
    }

    pub fn take_batch(self) -> Vec<Batch> {
        let mut result = Vec::with_capacity(self.batches.len());

        for batch in self.batches {
            result.push(batch.into_inner());
        }

        result
    }
}

struct MtSnapshotData {
    /// The snapshot data of each table.
    /// The box is supposed to be SnapshotData<K, V>.
    tables: Vec<Option<Box<dyn Any>>>,
}

pub struct TableRef<
    'selector,
    K: Hash + Eq + Serialize + DeserializeOwned + Any,
    V: Serialize + DeserializeOwned + Any,
    S: SerdeBackend,
> {
    tid: TableId,
    batch: SnapshotData<K, V>,
    selector: &'selector TableSelector<S>,
}

#[derive(Clone, Copy)]
pub struct ResolvedTableReference<K, V> {
    atomo_id: usize,
    index: TableId,
    kv: PhantomData<(K, V)>,
}

impl<K, V> ResolvedTableReference<K, V> {
    fn new(atomo_id: usize, index: TableId) -> Self {
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

        TableRef {
            tid: self.index,
            batch: SnapshotData::default(),
            selector,
        }
    }
}

impl GcNode<MtSnapshotData> {
    fn get<K, V>(&self, tid: TableId, key: &K) -> Option<Option<&V>>
    where
        K: Hash + Eq + Serialize + DeserializeOwned + Any,
        V: Serialize + DeserializeOwned + Any,
    {
        let mut current = self;

        loop {
            if let Some(Some(entries)) = current.value.load().map(|d| &d.tables[tid as usize]) {
                let entries: &SnapshotData<K, V> =
                    entries.downcast_ref().expect("Unexpected type error");

                match entries.0.get(key) {
                    Some(Operation::Put(v)) => return Some(Some(v)),
                    Some(Operation::Delete) => return Some(None),
                    None => {},
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

impl<'selector, K, V, S: SerdeBackend> TableRef<'selector, K, V, S>
where
    K: Hash + Eq + Serialize + DeserializeOwned + Any,
    V: Serialize + DeserializeOwned + Any,
{
    pub fn insert(&mut self, key: K, value: V) {
        self.batch.0.insert(key, Operation::Put(value));
    }

    pub fn remove(&mut self, key: K) {
        self.batch.0.insert(key, Operation::Delete);
    }

    pub fn get(&self, key: &K) -> Option<Shared<V>> {
        match self.batch.0.get(key) {
            Some(Operation::Put(value)) => return Some(Shared::new(value)),
            Some(Operation::Delete) => return None,
            _ => {},
        }

        if let Some(data) = self.selector.snapshot.get::<K, V>(self.tid, key) {
            return data.map(|v| Shared::new(v));
        }

        self.selector
            .atomo
            .get::<K, V>(self.tid, key)
            .map(|v| Shared::owned(v))
    }

    fn compute_inverse_and_batch(&mut self, batch: &mut Batch) -> Box<dyn Any> {
        let entries = std::mem::take(&mut self.batch).0;
        let cap = entries.len();

        let mut diff = FxHashMap::<K, Operation<V>>::with_capacity_and_hasher(
            cap,
            fxhash::FxBuildHasher::default(),
        );

        batch.reserve(cap);

        for (key, op) in entries {
            let key_ser = S::serialize(&key);

            match (self.selector.atomo.get_raw(self.tid, &key_ser), op) {
                (Some(old_value_ser), Operation::Put(new_value)) => {
                    let new_value_ser = S::serialize(&new_value);

                    if old_value_ser != new_value_ser {
                        let key_boxed = key_ser.into_boxed_slice();
                        let value_boxed = new_value_ser.into_boxed_slice();
                        batch.push((key_boxed, Some(value_boxed)));

                        let value = S::deserialize(&old_value_ser);
                        diff.insert(key, Operation::Put(value));
                    }
                },
                (Some(old_value_ser), Operation::Delete) => {
                    let key_boxed = key_ser.into_boxed_slice();
                    batch.push((key_boxed, None));

                    let value = S::deserialize(&old_value_ser);
                    diff.insert(key, Operation::Put(value));
                },
                (None, Operation::Put(new_value)) => {
                    let new_value_ser = S::serialize(&new_value);
                    let key_boxed = key_ser.into_boxed_slice();
                    let value_boxed = new_value_ser.into_boxed_slice();
                    batch.push((key_boxed, Some(value_boxed)));

                    diff.insert(key, Operation::Delete);
                },
                (None, Operation::Delete) => {
                    // Not a change.
                },
            }
        }

        Box::new(diff)
    }
}

impl<'selector, K, V, S: SerdeBackend> Drop for TableRef<'selector, K, V, S>
where
    K: Hash + Eq + Serialize + DeserializeOwned + Any,
    V: Serialize + DeserializeOwned + Any,
{
    fn drop(&mut self) {
        if let Some(inv_batch_list) = &self.selector.inv_batch_list {
            let inv = self.compute_inverse_and_batch(
                inv_batch_list.batches[self.tid as usize]
                    .borrow_mut()
                    .as_mut(),
            );

            *inv_batch_list.inverse[self.tid as usize].borrow_mut() = Some(inv);
        }
    }
}
