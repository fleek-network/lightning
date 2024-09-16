use std::io;

use crate::bucket::{errors, Bucket};
use crate::entry::BorrowedEntry;

pub struct UntrustedDirWriter {}

impl UntrustedDirWriter {
    pub fn new(bucket: &Bucket, num_entries: usize, hash: [u8; 32]) -> Self {
        todo!()
    }

    pub async fn feed_proof(&mut self, proof: &[u8]) -> Result<(), errors::FeedProofError> {
        todo!()
    }

    pub async fn insert<'a>(
        &mut self,
        entry: impl Into<BorrowedEntry<'a>>,
    ) -> Result<(), errors::InsertError> {
        todo!()
    }

    /// Finalize this write and flush the data to the disk.
    pub async fn commit(self) -> Result<[u8; 32], errors::CommitError> {
        todo!()
    }

    /// Cancel this write and remove anything that this writer wrote to the disk.
    pub async fn rollback(self) -> Result<(), io::Error> {
        todo!()
    }
}
