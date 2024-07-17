use fmmap::tokio::AsyncMmapFileExt;

use super::header::Header;
use crate::collections::{FlatHashSlice, HashTree};

pub struct File {
    header: Header,
}

impl File {
    pub(crate) fn new(header: Header) -> Self {
        Self { header }
    }

    /// Returns the hashtree of this file. The returned hash tree will be backed by a memory
    /// mapping.
    pub fn get_hashtree(&self) -> HashTree<'_> {
        let end = self.header.content.len();
        let slice = FlatHashSlice::from_tokio_mmap(4, end, &self.header.content);
        HashTree::try_from(slice).unwrap()
    }
}
