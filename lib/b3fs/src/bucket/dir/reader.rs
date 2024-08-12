use super::iter::DirEntriesIter;
use crate::collections::HashTree;
use crate::directory::BorrowedEntry;

pub struct Dir {}

impl Dir {
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
