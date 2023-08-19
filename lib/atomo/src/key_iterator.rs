use std::hash::Hash;
use std::marker::PhantomData;

use serde::de::DeserializeOwned;

use crate::batch::BoxedVec;
use crate::keys::ImKeyCollection;
use crate::{DefaultSerdeBackend, SerdeBackend};

/// An iterator over the keys of the table.
pub struct KeyIterator<K, S: SerdeBackend = DefaultSerdeBackend> {
    inner: im::hashset::ConsumingIter<BoxedVec>,
    phantom: PhantomData<(K, S)>,
}

impl<K, S: SerdeBackend> KeyIterator<K, S> {
    pub(crate) fn new(keys: ImKeyCollection) -> Self {
        Self {
            inner: keys.into_iter(),
            phantom: PhantomData,
        }
    }
}

impl<K, S: SerdeBackend> Iterator for KeyIterator<K, S>
where
    K: Hash + Eq + DeserializeOwned,
{
    type Item = K;

    fn next(&mut self) -> Option<Self::Item> {
        self.inner.next().map(|x| S::deserialize(&x))
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.inner.size_hint()
    }
}
