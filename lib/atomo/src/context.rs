use std::{hash::Hash, sync::Arc};

use serde::{de::DeserializeOwned, Serialize};

use crate::db::AtomoInner;
use crate::shared::Shared;
use crate::snapshot::{Snapshot, SnapshotData};
use crate::SerdeBackend;

pub struct Context<K, V, S: SerdeBackend> {
    atomo: Arc<AtomoInner<K, V, S>>,
    snapshot: Snapshot<K, V>,
}

impl<K, V, S: SerdeBackend> Context<K, V, S>
where
    K: Hash + Eq + Serialize + DeserializeOwned,
    V: Serialize + DeserializeOwned,
{
    pub(crate) fn new(atomo: Arc<AtomoInner<K, V, S>>, snapshot: Arc<Snapshot<K, V>>) -> Self {
        Self {
            atomo,
            snapshot: Snapshot::with_value_and_next(SnapshotData::default(), snapshot),
        }
    }

    pub(crate) fn into_snapshot(self) -> Snapshot<K, V> {
        self.snapshot
    }

    /// Returns the value associated with the given key.
    pub fn get(&self, key: &K) -> Option<Shared<V>> {
        if let Some(value) = self.snapshot.get(key) {
            return value.map(|v| Shared::new(v));
        }

        self.atomo.get(key).map(|v| Shared::owned(v))
    }

    /// Insert the given key value pair into the current state.
    pub fn insert(&mut self, key: K, value: V) {
        self.snapshot.insert(key, value);
    }

    /// Remove the given key from the current state.
    pub fn remove(&mut self, key: K) {
        self.snapshot.remove(key);
    }

    pub fn scoped<F, T, E>(&mut self, _subtask: F) -> Result<T, E>
    where
        F: Fn(&mut Context<K, V, S>) -> Result<T, E>,
    {
        todo!()
    }
}
