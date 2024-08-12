use std::any::Any;
use std::hash::Hash;
use std::marker::PhantomData;
use std::sync::Arc;

use serde::de::DeserializeOwned;
use serde::Serialize;

use crate::inner::AtomoInner;
use crate::serder::SerdeBackend;
use crate::storage::{InMemoryStorage, StorageBackend};
use crate::table::{ResolvedTableReference, TableSelector};
use crate::DefaultSerdeBackend;

/// The id of a table. Currently we are limited to 256 tables which is something
/// I believe to be reasonable limitation for the use cases we care to support.
pub type TableId = u8;

/// The query permission on an [`Atomo`] only allows non-mutating changes.
pub struct QueryPerm;

/// The update permission on an [`Atomo`] which allows mutating the data.
pub struct UpdatePerm;

/// Atomo is the concurrent query and update execution engine that can allow many queries to
/// run in parallel while a single update thread takes ownership of mutating the states.
///
/// This is achieved using the permission markers [`QueryPerm`] and [`UpdatePerm`]. When a
/// new [`Atomo`] instance is created that instance is [`Atomo<UpdatePerm>`] and has the
/// ability to run mutating changes on the database. There can only ever be one mutable
/// reference to the database (i.e one [`Atomo<UpdatePerm>`]) and if it's lost there is no
/// way to get it back.
///
/// But we allow many parallel access to the same data. An Atomo with update permission
/// can be downgraded to an [`Atomo<QueryPerm>`] using the [`Atomo::query`] method.
///
/// An important note here is that the [`Atomo<QueryPerm>`] implements [`Clone`] so that
/// you can clone it anytime (and the clone implementation is rather cheap.)
pub struct Atomo<O, B: StorageBackend = InMemoryStorage, S: SerdeBackend = DefaultSerdeBackend> {
    inner: Arc<AtomoInner<B, S>>,
    ownership: PhantomData<O>,
}

// only implement the clone for the query permission.
impl<B: StorageBackend, S: SerdeBackend> Clone for Atomo<QueryPerm, B, S> {
    fn clone(&self) -> Self {
        Self::new(self.inner.clone())
    }
}

impl<O, B: StorageBackend, S: SerdeBackend> Atomo<O, B, S> {
    #[inline]
    pub(crate) fn new(inner: Arc<AtomoInner<B, S>>) -> Self {
        Self {
            inner,
            ownership: PhantomData,
        }
    }

    /// Returns a query end for this table.
    pub fn query(&self) -> Atomo<QueryPerm, B, S> {
        Atomo::new(self.inner.clone())
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

    /// Returns the list of open tables.
    #[inline]
    pub fn tables(&self) -> Vec<String> {
        self.inner
            .tables
            .iter()
            .map(|table| table.name.clone())
            .collect()
    }
}

impl<B: StorageBackend, S: SerdeBackend> Atomo<QueryPerm, B, S> {
    /// Run a query on the database.
    pub fn run<F, R>(&self, query: F) -> R
    where
        F: FnOnce(&mut TableSelector<B, S>) -> R,
    {
        let mut selector = TableSelector::new(self.inner.clone());
        query(&mut selector)
    }
}

impl<B: StorageBackend, S: SerdeBackend> Atomo<UpdatePerm, B, S> {
    /// Run an update on the database.
    pub fn run<F, R>(&mut self, mutation: F) -> R
    where
        F: FnOnce(&mut TableSelector<B, S>) -> R,
    {
        let mut selector = TableSelector::new(self.inner.clone());
        let response = mutation(&mut selector);

        let (batch, keys) = selector.into_raw();
        let inverse = self.inner.compute_inverse(&batch);
        self.inner.snapshot_list.push(inverse, keys, || {
            self.inner.perform_batch(batch);
        });

        response
    }

    /// Return a reference to the storage backend. Modifying the state directly and going behind
    /// Atomo will break Atomo.
    ///
    /// Only use this when you need an special read operation on the underlying storage and do
    /// not perform any updates to the database.
    ///
    /// This method is not marked as unsafe since it does not allow you to violate borrow checker,
    /// but it should be treated as such.
    pub fn get_storage_backend_unsafe(&mut self) -> &B {
        &self.inner.persistence
    }
}

mod doc_tests {
    /// This should compile fine since it is an attempt to clone an Atomo instance with
    /// query permissions.
    ///
    /// ```
    /// use atomo::*;
    ///
    /// fn is_clone<T: Clone>() {}
    ///
    /// fn ensure_atomo_is_clone<B: StorageBackend, S: SerdeBackend>() {
    ///     is_clone::<Atomo<QueryPerm, B, S>>()
    /// }
    /// ```
    ///
    /// This compilation MUST fail since we don't want UpdatePerm to allow for clone.
    ///
    /// ```compile_fail
    /// use atomo::*;
    ///
    /// fn is_clone<T: Clone>() {}
    ///
    /// fn ensure_atomo_is_clone<B: StorageBackend, S: SerdeBackend>() {
    ///     is_clone::<Atomo<UpdatePerm, B, S>>()
    /// }
    /// ```
    fn _ensure_update_perm_not_clone() {}
}
