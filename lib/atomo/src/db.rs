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
    pub fn run<F, R>(&self, _query: F) -> R
    where
        F: Fn(&mut TableSelector<S>) -> R,
    {
        todo!()
    }
}

impl<S: SerdeBackend> Atomo<UpdatePerm, S> {
    pub fn run<F, R>(&mut self, _mutation: F) -> R
    where
        F: Fn(&mut TableSelector<S>) -> R,
    {
        todo!()
    }
}
