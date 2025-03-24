//! Forms a post-order binary tree over a flat hash slice.

use core::hash;
use std::cmp::min;
use std::fmt::Debug;
use std::future::Future;
use std::io::Read;
use std::mem::{self, MaybeUninit};
use std::ops::Index;
use std::path::PathBuf;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};

use fleek_blake3::tree;

use super::error::CollectionTryFromError;
use super::flat::FlatHashSlice;
#[cfg(not(feature = "sync"))]
use crate::bucket::errors::ReadError;
#[cfg(not(feature = "sync"))]
use crate::bucket::POSITION_START_HASHES;
use crate::entry::BorrowedEntry;
use crate::hasher;
use crate::hasher::dir_hasher::DirectoryHasher;
use crate::stream::buffer::ProofBuf;
use crate::stream::walker::{self, Mode, TreeWalker};
use crate::stream::ProofEncoder;
#[cfg(feature = "sync")]
use crate::sync::bucket::errors::ReadError;
#[cfg(feature = "sync")]
use crate::sync::bucket::POSITION_START_HASHES;
use crate::utils::{block_counter_from_tree_index, is_valid_tree_len, tree_index};

/// A wraipper around a list of hashes that provides access only to the leaf nodes in the tree.
#[derive(Clone, Copy)]
pub struct HashTree<'s> {
    inner: FlatHashSlice<'s>,
}

/// An iterator over a [`HashTree`] which iterates over the leaf nodes of a tree.
pub struct HashTreeIter<'t> {
    forward: usize,
    backward: usize,
    tree: HashTree<'t>,
}

impl<'s> TryFrom<FlatHashSlice<'s>> for HashTree<'s> {
    type Error = CollectionTryFromError;

    #[inline]
    fn try_from(value: FlatHashSlice<'s>) -> Result<Self, Self::Error> {
        if !is_valid_tree_len(value.len()) {
            Err(CollectionTryFromError::InvalidHashCount)
        } else {
            Ok(Self { inner: value })
        }
    }
}

impl<'s> TryFrom<&'s [u8]> for HashTree<'s> {
    type Error = CollectionTryFromError;

    #[inline]
    fn try_from(value: &'s [u8]) -> Result<Self, Self::Error> {
        Self::try_from(FlatHashSlice::try_from(value)?)
    }
}

impl<'s> TryFrom<&'s [[u8; 32]]> for HashTree<'s> {
    type Error = CollectionTryFromError;

    #[inline]
    fn try_from(value: &'s [[u8; 32]]) -> Result<Self, Self::Error> {
        Self::try_from(FlatHashSlice::from(value))
    }
}

impl<'s> TryFrom<&'s Vec<[u8; 32]>> for HashTree<'s> {
    type Error = CollectionTryFromError;

    #[inline]
    fn try_from(value: &'s Vec<[u8; 32]>) -> Result<Self, Self::Error> {
        Self::try_from(FlatHashSlice::from(value))
    }
}

impl Index<usize> for HashTree<'_> {
    type Output = [u8; 32];

    #[inline]
    fn index(&self, index: usize) -> &Self::Output {
        if index >= self.len() {
            // TODO(qti3e): is this check necessary?
            panic!("Out of bound.");
        }

        &self.inner[tree_index(index)]
    }
}

impl<'s> IntoIterator for HashTree<'s> {
    type Item = &'s [u8; 32];
    type IntoIter = HashTreeIter<'s>;
    fn into_iter(self) -> Self::IntoIter {
        HashTreeIter::new(self)
    }
}

impl<'s> HashTree<'s> {
    #[inline]
    pub fn root(&self) -> &'s [u8; 32] {
        self.inner.get(self.inner.len() - 1)
    }

    #[inline]
    pub fn nth(&self, index: usize) -> &'s [u8; 32] {
        self.inner.get(tree_index(index))
    }

    /// Returns the number of items in this hash tree.
    #[inline]
    pub fn len(&self) -> usize {
        (self.inner.len() + 1) >> 1
    }

    /// Returns the total number of hashes making up this tree.
    #[inline]
    pub fn inner_len(&self) -> usize {
        self.inner.len()
    }

    /// A hash tree is never empty.
    #[inline(always)]
    pub fn is_empty(&self) -> bool {
        false
    }

    /// Returns the internal representation of the hash tree which is a flat hash slice.
    #[inline(always)]
    pub fn as_inner(&self) -> &FlatHashSlice {
        &self.inner
    }

    /// Shorthand for [`ProofBuf::new`].
    #[inline]
    pub fn generate_proof(&self, mode: Mode, index: usize) -> ProofBuf {
        ProofBuf::new(mode, *self, index)
    }
}

impl<'s> HashTreeIter<'s> {
    fn new(tree: HashTree<'s>) -> Self {
        Self {
            forward: 0,
            backward: tree.len(),
            tree,
        }
    }

    #[inline(always)]
    fn is_done(&self) -> bool {
        self.forward >= self.backward
    }
}

impl<'s> Iterator for HashTreeIter<'s> {
    type Item = &'s [u8; 32];

    fn next(&mut self) -> Option<Self::Item> {
        if self.is_done() {
            return None;
        }
        let idx = tree_index(self.forward);
        self.forward += 1;
        Some(self.tree.inner.get(idx))
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let r = self.backward.saturating_sub(self.forward);
        (r, Some(r))
    }

    fn nth(&mut self, n: usize) -> Option<Self::Item> {
        self.forward += n;
        self.next()
    }
}

impl DoubleEndedIterator for HashTreeIter<'_> {
    fn next_back(&mut self) -> Option<Self::Item> {
        if self.is_done() {
            return None;
        }
        self.backward -= 1;
        let idx = tree_index(self.backward);
        Some(self.tree.inner.get(idx))
    }

    fn nth_back(&mut self, n: usize) -> Option<Self::Item> {
        self.backward = self.backward.saturating_sub(n);
        self.next()
    }
}

impl Debug for HashTree<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        super::printer::print(self, f)
    }
}
