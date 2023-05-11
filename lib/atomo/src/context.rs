use std::fmt::{Debug, Display};
use std::ops::Deref;
use std::{hash::Hash, sync::Arc};

use serde::{de::DeserializeOwned, Serialize};

use crate::atomo::AtomoInner;
use crate::snapshot::Snapshot;
use crate::SerdeBackend;

pub struct Context<K, V, S: SerdeBackend> {
    atomo: Arc<AtomoInner<K, V, S>>,
    snapshot: Snapshot<K, V>,
}

pub enum Shared<'a, T> {
    Reference(&'a T),
    Owned(T),
}

impl<K, V, S: SerdeBackend> Context<K, V, S>
where
    K: Hash + Eq + Serialize + DeserializeOwned,
    V: Serialize + DeserializeOwned,
{
    pub(crate) fn new(atomo: Arc<AtomoInner<K, V, S>>, snapshot: Arc<Snapshot<K, V>>) -> Self {
        Self {
            atomo,
            snapshot: Snapshot::with_empty_entries_and_next(snapshot),
        }
    }

    pub(crate) fn into_snapshot(self) -> Snapshot<K, V> {
        self.snapshot
    }

    /// Returns the value associated with the given key.
    pub fn get(&self, key: &K) -> Option<Shared<V>> {
        if let Some(value) = self.snapshot.get(key) {
            return value.map(|v| Shared::Reference(v));
        }

        self.atomo.get(key).map(|v| Shared::Owned(v))
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

impl<'a, T> Deref for Shared<'a, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        match self {
            Self::Reference(t) => *t,
            Self::Owned(t) => t,
        }
    }
}

impl<'a, T> AsRef<T> for Shared<'a, T> {
    fn as_ref(&self) -> &T {
        match self {
            Self::Reference(t) => *t,
            Self::Owned(t) => t,
        }
    }
}

impl<'a, T: Clone> Clone for Shared<'a, T> {
    fn clone(&self) -> Self {
        match self {
            Self::Reference(t) => Self::Reference(*t),
            Self::Owned(t) => Self::Owned(t.clone()),
        }
    }
}

impl<'a, T: Debug> Debug for Shared<'a, T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Reference(t) => (*t).fmt(f),
            Self::Owned(t) => t.fmt(f),
        }
    }
}

impl<'a, T: Display> Display for Shared<'a, T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Reference(t) => (*t).fmt(f),
            Self::Owned(t) => t.fmt(f),
        }
    }
}

impl<'a, T: Hash> Hash for Shared<'a, T> {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.as_ref().hash(state)
    }
}

impl<'a, T: PartialEq> PartialEq<Shared<'a, T>> for Shared<'a, T> {
    #[inline]
    fn eq(&self, other: &Shared<'a, T>) -> bool {
        self.as_ref().eq(other.as_ref())
    }

    #[inline]
    fn ne(&self, other: &Shared<'a, T>) -> bool {
        self.as_ref().ne(other.as_ref())
    }
}

impl<'a, T: PartialEq> PartialEq<T> for Shared<'a, T> {
    #[inline]
    fn eq(&self, other: &T) -> bool {
        self.as_ref().eq(other)
    }

    #[inline]
    fn ne(&self, other: &T) -> bool {
        self.as_ref().ne(other)
    }
}

impl<'a, T: PartialOrd> PartialOrd<Shared<'a, T>> for Shared<'a, T> {
    #[inline]
    fn partial_cmp(&self, other: &Shared<'a, T>) -> Option<std::cmp::Ordering> {
        self.as_ref().partial_cmp(other.as_ref())
    }

    #[inline]
    fn lt(&self, other: &Shared<'a, T>) -> bool {
        self.as_ref().lt(other.as_ref())
    }

    #[inline]
    fn le(&self, other: &Shared<'a, T>) -> bool {
        self.as_ref().le(other.as_ref())
    }

    #[inline]
    fn gt(&self, other: &Shared<'a, T>) -> bool {
        self.as_ref().gt(other.as_ref())
    }

    #[inline]
    fn ge(&self, other: &Shared<'a, T>) -> bool {
        self.as_ref().ge(other.as_ref())
    }
}

impl<'a, T: PartialOrd> PartialOrd<T> for Shared<'a, T> {
    #[inline]
    fn partial_cmp(&self, other: &T) -> Option<std::cmp::Ordering> {
        self.as_ref().partial_cmp(other)
    }

    #[inline]
    fn lt(&self, other: &T) -> bool {
        self.as_ref().lt(other)
    }

    #[inline]
    fn le(&self, other: &T) -> bool {
        self.as_ref().le(other)
    }

    #[inline]
    fn gt(&self, other: &T) -> bool {
        self.as_ref().gt(other)
    }

    #[inline]
    fn ge(&self, other: &T) -> bool {
        self.as_ref().ge(other)
    }
}

impl<'a, T: Eq> Eq for Shared<'a, T> {}

impl<'a, T: Ord> Ord for Shared<'a, T> {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.as_ref().cmp(other.as_ref())
    }
}
