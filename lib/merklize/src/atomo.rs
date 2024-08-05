use std::any::Any;
use std::hash::Hash;
use std::marker::PhantomData;

use atomo::{
    Atomo,
    QueryPerm,
    ResolvedTableReference,
    SerdeBackend,
    StorageBackend,
    TableSelector,
    UpdatePerm,
};
use serde::de::DeserializeOwned;
use serde::Serialize;

use crate::MerklizeProvider;

/// A merklize atomo that can be used to read and update tables of data, wrapping an
/// `[atomo::Atomo]` instance to provide similar functionality, but with additional merklize state
/// tree features.
///
/// Most methods are passthroughs to the inner atomo instance, but the `run` method on the
/// `UpdatePerm` instance is augmented to derive and apply the state tree changes based on the state
/// changes. The `QueryPerm` implementation also provides additional methods to read the state tree,
/// such as `get_state_root` and `get_state_proof`.
///
/// ## Example
///
/// ```rust
#[doc = include_str!("../examples/jmt-sha256.rs")]
/// ```
pub struct MerklizedAtomo<P, B: StorageBackend, S: SerdeBackend, M: MerklizeProvider> {
    inner: Atomo<P, B, S>,
    _phantom: PhantomData<M>,
}

impl<B: StorageBackend, S: SerdeBackend, M: MerklizeProvider> Clone
    for MerklizedAtomo<QueryPerm, B, S, M>
{
    fn clone(&self) -> Self {
        Self::new(self.inner.clone())
    }
}

impl<P, B: StorageBackend, S: SerdeBackend, M: MerklizeProvider> MerklizedAtomo<P, B, S, M> {
    /// Create a new merklize atomo.
    pub fn new(inner: Atomo<P, B, S>) -> Self {
        Self {
            inner,
            _phantom: PhantomData,
        }
    }

    /// Build and return a query reader for the data.
    /// This is a passthrough to the inner atomo instance, but wraps the result in
    /// `[merklize::MerklizedAtomo<QueryPerm>]` so that it can also provide additional
    /// merklize state tree features.
    #[inline]
    pub fn query(&self) -> Atomo<QueryPerm, B, S> {
        self.inner.query()
    }

    /// Resolve a table with the given name and key-value types.
    /// This is a direct passthrough to the inner atomo instance.
    #[inline]
    pub fn resolve<K, V>(&self, name: impl AsRef<str>) -> ResolvedTableReference<K, V>
    where
        K: Hash + Eq + Serialize + DeserializeOwned + Any,
        V: Serialize + DeserializeOwned + Any,
    {
        self.inner.resolve::<K, V>(name.as_ref())
    }
}

impl<B: StorageBackend, S: SerdeBackend, M: MerklizeProvider<Storage = B, Serde = S>>
    MerklizedAtomo<UpdatePerm, B, S, M>
{
    /// Run an update on the data using the wrapped atomo instance, then derive and apply the state
    /// tree changes based on the state changes. This ensures that the state tree is in sync and
    /// consistent with the state data.
    pub fn run<F, R>(&mut self, mutation: F) -> R
    where
        F: FnOnce(&mut TableSelector<B, S>) -> R,
    {
        self.inner.run(|ctx| {
            let res = mutation(ctx);

            // Apply the state tree changes based on the state changes.
            M::apply_state_tree_changes(ctx).unwrap();

            res
        })
    }

    /// Return the internal storage backend.
    /// This is a direct passthrough to the inner atomo instance.
    pub fn get_storage_backend_unsafe(&mut self) -> &B {
        self.inner.get_storage_backend_unsafe()
    }
}

impl<B: StorageBackend, S: SerdeBackend, M: MerklizeProvider<Storage = B, Serde = S>>
    MerklizedAtomo<QueryPerm, B, S, M>
{
    /// Run a query on the database.
    /// This is a direct passthrough to the inner atomo instance.
    pub fn run<F, R>(&self, query: F) -> R
    where
        F: FnOnce(&mut TableSelector<B, S>) -> R,
    {
        self.inner.run(|ctx| query(ctx))
    }
}
