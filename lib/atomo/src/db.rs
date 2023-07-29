use std::{any::Any, hash::Hash, marker::PhantomData, sync::Arc};

use serde::{de::DeserializeOwned, Serialize};

use crate::{
    inner::AtomoInner,
    serder::SerdeBackend,
    table::{ResolvedTableReference, TableSelector},
    DefaultSerdeBackend,
};

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
pub struct Atomo<O, S: SerdeBackend = DefaultSerdeBackend> {
    inner: Arc<AtomoInner<S>>,
    ownership: PhantomData<O>,
}

// only implement the clone for the query permission.
impl<S: SerdeBackend> Clone for Atomo<QueryPerm, S> {
    fn clone(&self) -> Self {
        Self::new(self.inner.clone())
    }
}

impl<O, S: SerdeBackend> Atomo<O, S> {
    #[inline]
    pub(crate) fn new(inner: Arc<AtomoInner<S>>) -> Self {
        Self {
            inner,
            ownership: PhantomData,
        }
    }

    /// Returns a query end for this table.
    pub fn query(&self) -> Atomo<QueryPerm, S> {
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
}

impl<S: SerdeBackend> Atomo<QueryPerm, S> {
    /// Run a query on the database.
    pub fn run<F, R>(&self, query: F) -> R
    where
        F: Fn(&mut TableSelector<S>) -> R,
    {
        let mut selector = TableSelector::new(self.inner.clone());
        query(&mut selector)
    }
}

impl<S: SerdeBackend> Atomo<UpdatePerm, S> {
    /// Run an update on the database.
    pub fn run<F, R>(&mut self, mutation: F) -> R
    where
        F: Fn(&mut TableSelector<S>) -> R,
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
    /// fn ensure_atomo_is_clone<S: SerdeBackend>() {
    ///     is_clone::<Atomo<QueryPerm, S>>()
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
    /// fn ensure_atomo_is_clone<S: SerdeBackend>() {
    ///     is_clone::<Atomo<UpdatePerm, S>>()
    /// }
    /// ```
    fn _ensure_update_perm_not_clone() {}
}
