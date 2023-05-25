use std::{hash::Hash, marker::PhantomData, sync::Arc};

use dashmap::DashMap;
use fxhash::FxHashMap;
use serde::{de::DeserializeOwned, Serialize};

use crate::{
    context::Context,
    gc_list::GcList,
    snapshot::{Snapshot, SnapshotData},
    DefaultSerdeBackend, SerdeBackend,
};

pub struct Atomo<K, V, S: SerdeBackend = DefaultSerdeBackend> {
    inner: Arc<AtomoInner<K, V, S>>,
}

pub type Batch = Vec<(Box<[u8]>, Option<Box<[u8]>>)>;

pub(crate) struct AtomoInner<K, V, S: SerdeBackend> {
    persistence: DashMap<Box<[u8]>, Box<[u8]>, fxhash::FxBuildHasher>,
    head: GcList<SnapshotData<K, V>>,
    serde: PhantomData<S>,
}

pub(crate) enum Operation<V> {
    Put(V),
    Delete,
}

/// The query end of an [`Atomo`] which can be used to run many queries concurrently and in
/// parallel. You can `clone` a [`QueryHalf`] as much as you want and pass it around.
pub struct QueryHalf<K, V, S: SerdeBackend>(Arc<AtomoInner<K, V, S>>);

/// The update end of an [`Atomo`]. There can only ever be a maximum of one [`UpdateHalf`]
/// since we need to guarantee one writer exists.
pub struct UpdateHalf<K, V, S: SerdeBackend>(Arc<AtomoInner<K, V, S>>);

impl<K, V, S: SerdeBackend> Clone for QueryHalf<K, V, S> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

impl<K, V, S: SerdeBackend> Atomo<K, V, S> {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(AtomoInner {
                persistence: DashMap::default(),
                head: GcList::new(),
                serde: PhantomData,
            }),
        }
    }

    /// Split this [`Atomo`] environment to the query and update half.
    pub fn split(self) -> (UpdateHalf<K, V, S>, QueryHalf<K, V, S>) {
        (UpdateHalf(self.inner.clone()), QueryHalf(self.inner))
    }
}

impl<K, V, S: SerdeBackend> AtomoInner<K, V, S>
where
    K: Hash + Eq + Serialize + DeserializeOwned,
    V: Serialize + DeserializeOwned,
{
    /// Return a value associated with the key.
    pub fn get(&self, key: &K) -> Option<V> {
        let key_serialized = S::serialize(key);
        self.get_raw(&key_serialized).map(|v| S::deserialize(&v))
    }

    /// Return the raw byte representation of a value for a raw key.
    pub fn get_raw(&self, key: &[u8]) -> Option<Vec<u8>> {
        self.persistence.get(key).map(|v| v.to_vec())
    }

    /// Performs a batch of operations on the persistence layer.
    fn perform_batch(&self, batch: Batch) {
        for (k, op) in batch {
            match op {
                Some(value) => {
                    self.persistence.insert(k, value);
                },
                None => {
                    self.persistence.remove(&k);
                },
            }
        }
    }

    /// Given a wish-to-be-executed set of changes, returns the *inverse* of these changes (which
    /// we can put to a snapshot) and the batch of changes that should be sent to the persistence.
    fn generate_inverse_and_batch(
        &self,
        entries: FxHashMap<K, Operation<V>>,
    ) -> (FxHashMap<K, Operation<V>>, Batch) {
        let cap = entries.len();

        let mut diff = FxHashMap::<K, Operation<V>>::with_capacity_and_hasher(
            cap,
            fxhash::FxBuildHasher::default(),
        );

        let mut batch = Batch::with_capacity(cap);

        for (key, op) in entries {
            let key_ser = S::serialize(&key);

            match (self.get_raw(&key_ser), op) {
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

        (diff, batch)
    }

    pub fn run<F, R>(this: &Arc<Self>, closure: F) -> (R, Snapshot<K, V>)
    where
        F: Fn(&mut Context<K, V, S>) -> R,
    {
        let snapshot = this.head.current();
        let mut ctx = Context::new(this.clone(), snapshot);
        let result = closure(&mut ctx);
        (result, ctx.into_snapshot())
    }
}

impl<K, V, S: SerdeBackend> QueryHalf<K, V, S>
where
    K: Hash + Eq + Serialize + DeserializeOwned,
    V: Serialize + DeserializeOwned,
{
    pub fn run<F, R>(&self, query: F) -> R
    where
        F: Fn(&mut Context<K, V, S>) -> R,
    {
        let (res, _) = AtomoInner::run(&self.0, query);
        res
    }
}

impl<K, V, S: SerdeBackend> UpdateHalf<K, V, S>
where
    K: Hash + Eq + Serialize + DeserializeOwned,
    V: Serialize + DeserializeOwned,
{
    /// Create and return a [`QueryHalf`] from this [`UpdateHalf`].
    pub fn downgrade(&self) -> QueryHalf<K, V, S> {
        QueryHalf(self.0.clone())
    }

    pub fn run<F, R>(&mut self, mutation: F) -> R
    where
        F: Fn(&mut Context<K, V, S>) -> R,
    {
        let (res, snap) = AtomoInner::run(&self.0, mutation);

        let entries = snap.into_value().unwrap().0;
        let (inverse, batch) = self.0.generate_inverse_and_batch(entries);

        // SAFETY: We are the only one calling this method.
        let next = unsafe { self.0.head.push(SnapshotData(inverse)) };

        self.0.perform_batch(batch);

        // SAFETY: We are calling this immediately after `push`.
        unsafe {
            self.0.head.set_head(next);
        }

        res
    }
}
