use std::{hash::Hash, marker::PhantomData};

use serde::de::DeserializeOwned;

use crate::{batch::BoxedVec, keys::ImKeyCollection, DefaultSerdeBackend, SerdeBackend};

/// An iterator over the keys of the table.
pub struct KeyIterator<'it, K, S: SerdeBackend = DefaultSerdeBackend> {
    inner: im::hashset::ConsumingIter<BoxedVec>,
    phantom: PhantomData<(&'it (), K, S)>,
}

impl<'it, K, S: SerdeBackend> KeyIterator<'it, K, S> {
    pub(crate) fn new(keys: ImKeyCollection) -> Self {
        Self {
            inner: keys.into_iter(),
            phantom: PhantomData,
        }
    }
}

impl<'it, K, S: SerdeBackend> Iterator for KeyIterator<'it, K, S>
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
