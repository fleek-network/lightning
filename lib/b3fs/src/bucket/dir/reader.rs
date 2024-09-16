use tokio::fs::{self};
use triomphe::Arc;

use super::iter::DirEntriesIter;
use crate::collections::HashTree;
use crate::entry::BorrowedEntry;

pub struct B3Dir {
    /// The number of entries in this directory.
    num_entries: u16,
    /// The actual header file.
    file: Arc<fs::File>,
}

impl B3Dir {
    pub(crate) fn new(num_entries: u32, file: Arc<fs::File>) -> Self {
        debug_assert!(num_entries <= (u16::MAX as u32));
        Self {
            num_entries: num_entries as u16,
            file,
        }
    }

    pub fn hashtree(&self) -> HashTree {
        todo!()
    }

    pub async fn get_entry(&self, name: &[u8]) -> Option<BorrowedEntry> {
        todo!()
    }

    pub fn entries(&self) -> DirEntriesIter {
        todo!()
    }
}
