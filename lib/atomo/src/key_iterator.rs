use std::{hash::Hash, marker::PhantomData};

use serde::de::DeserializeOwned;

use crate::{batch::BoxedVec, DefaultSerdeBackend, SerdeBackend};

pub type ImKeyCollection = im::HashSet<BoxedVec, fxhash::FxBuildHasher>;

/// An iterator over the keys of the table.
pub struct KeyIterator<'iter, K, S: SerdeBackend = DefaultSerdeBackend> {
    inner: im::hashset::Iter<'iter, BoxedVec>,
    phantom: PhantomData<(K, S)>,
}

impl<'iter, K> KeyIterator<'iter, K> {
    pub fn new(keys: &'iter ImKeyCollection) -> Self {
        Self {
            inner: keys.iter(),
            phantom: PhantomData,
        }
    }
}

impl<'iter, K, S: SerdeBackend> Iterator for KeyIterator<'iter, K, S>
where
    K: Hash + Eq + DeserializeOwned,
{
    type Item = K;

    fn next(&mut self) -> Option<Self::Item> {
        self.inner.next().map(|x| S::deserialize(x))
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.inner.size_hint()
    }
}
