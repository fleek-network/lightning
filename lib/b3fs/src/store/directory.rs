use fmmap::tokio::AsyncMmapFileExt;

use super::header::Header;
use crate::collections::{FlatHashSlice, HashTree};
use crate::directory::BorrowedEntry;
use crate::utils::is_valid_filename;

/// An open directory from the blockstore.
pub struct Directory {
    header: Header,
}

/// a
pub struct EntriesIterator<'d> {
    dir: &'d Directory,
}

impl Directory {
    pub(crate) fn new(header: Header) -> Self {
        Self { header }
    }

    /// Returns the number of entries in the directory.
    pub fn len(&self) -> u32 {
        let bytes = self.header.content.bytes(4, 4).unwrap();
        let array = arrayref::array_ref![bytes, 0, 4];
        u32::from_le_bytes(*array)
    }

    /// Returns the hashtree of the entries in this directory.
    pub fn get_hashtree(&self) -> HashTree<'_> {
        let n = (self.len().max(1) * 2 - 1) as usize;
        let slice = FlatHashSlice::from_tokio_mmap(8, n * 32 + 8, &self.header.content);
        HashTree::try_from(slice).unwrap()
    }

    /// Returns an iterator over the entries in this directory.
    pub fn iter(&self) -> EntriesIterator {
        EntriesIterator { dir: self }
    }

    /// Find and return an entry in this directory with the given name.
    pub fn find(&self, name: &[u8]) -> Option<BorrowedEntry> {
        if !is_valid_filename(name) {
            return None;
        }

        let len = self.len() as usize;
        let after_hashtree = (len.max(1) * 2 - 1) * 32 + 8;
        let offsets = self.header.content.bytes(after_hashtree, len * 4).unwrap();
        let file_len = self.header.content.len();

        todo!()
    }
}

/// Read a little endian [`u32`] from the given offset.
#[inline(always)]
fn read_u32(buffer: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes(*arrayref::array_ref![buffer, offset, 4])
}
