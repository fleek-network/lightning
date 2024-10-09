use std::io::{self, Write};

use crate::bucket::{errors, Bucket};
use crate::entry::BorrowedEntry;
use crate::hasher::byte_hasher::Blake3Hasher;
use crate::hasher::collector::BufCollector;
use crate::stream::verifier::{IncrementalVerifier, WithHashTreeCollector};

/// A trusted directory writer.
pub struct DirWriter {
    bucket: Bucket,
    num_entries: usize,
    hasher: Blake3Hasher<BufCollector>,
}

impl DirWriter {
    pub fn new(bucket: &Bucket, num_entries: usize) -> Self {
        Self {
            bucket: bucket.clone(),
            num_entries,
            hasher: Blake3Hasher::default(),
        }
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
