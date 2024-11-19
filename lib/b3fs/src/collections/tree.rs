//! Forms a post-order binary tree over a flat hash slice.

use std::cmp::min;
use std::fmt::Debug;
use std::future::Future;
use std::mem::{self, MaybeUninit};
use std::ops::Index;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};

use tokio::io::{AsyncReadExt, AsyncSeekExt};
use tokio::sync::RwLock;
use tokio_stream::Stream;

use super::error::CollectionTryFromError;
use super::flat::FlatHashSlice;
use crate::bucket::errors::ReadError;
use crate::bucket::POSITION_START_HASHES;
use crate::stream::buffer::ProofBuf;
use crate::stream::walker::Mode;
use crate::utils::{is_valid_tree_len, tree_index};

/// A wrapper around a list of hashes that provides access only to the leaf nodes in the tree.
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

impl<'s> Index<usize> for HashTree<'s> {
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

impl<'s> DoubleEndedIterator for HashTreeIter<'s> {
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

impl<'s> Debug for HashTree<'s> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        super::printer::print(self, f)
    }
}

pub struct AsyncHashTree<T: AsyncReadExt + AsyncSeekExt + Unpin> {
    file_reader: Arc<RwLock<T>>,
    number_of_hashes: u32,
    current_block: u32,
    pages: Vec<Option<Box<[[u8; 32]]>>>, // Store loaded pages as boxed slices
}

/// An asynchronous structure that reads hashes from memory pages.
impl<T> AsyncHashTree<T>
where
    T: AsyncReadExt + AsyncSeekExt + Unpin,
{
    pub fn new(file_reader: T, number_of_hashes: u32) -> Self {
        Self {
            file_reader: Arc::new(RwLock::new(file_reader)),
            number_of_hashes,
            current_block: 0,
            pages: vec![None; (number_of_hashes as usize + 1023) / 1024], // Initialize pages
        }
    }

    /// Asynchronously get the hash for the specified block number.
    pub async fn get_hash(&mut self, block_number: u32) -> Result<Option<[u8; 32]>, ReadError> {
        if block_number >= self.number_of_hashes {
            return Ok(None);
        }

        let tree_index = tree_index(block_number as usize);
        let page_index = (block_number as usize) / 1024;
        let offset = tree_index % 1024;

        // Load the page if it is not already loaded
        if self.pages[page_index].is_none() {
            let start_index = POSITION_START_HASHES as u64 + (page_index * 4096) as u64; // 4KB page size
            let file = self.file_reader.clone();
            let mut file_lock = file.write().await;

            // Determine the remaining bytes in the file
            let file_size = file_lock.seek(tokio::io::SeekFrom::End(0)).await?; // Get the file size
            file_lock
                .seek(tokio::io::SeekFrom::Start(start_index))
                .await?; // Seek back to the start index

            let bytes_to_read = (file_size - start_index) as usize;
            let mut page_data = vec![0; bytes_to_read.min(4096)]; // Create a buffer with the minimum of remaining bytes or 4096
            let bytes_read = file_lock.read_exact(&mut page_data).await?;

            let hashes: Vec<[u8; 32]> = page_data
                .chunks_exact(32)
                .map(|slice| {
                    let mut hash = [0; 32];
                    hash.copy_from_slice(slice);
                    hash
                })
                .collect();

            // Store the entire page of hashes
            self.pages[page_index] = Some(hashes.into_boxed_slice()); // Store as boxed slice of hashes
        }

        // Retrieve the hash from the loaded page
        let hashes = self.pages[page_index].as_ref();
        Ok(hashes.map(|x| x[offset]))
    }
}
