use std::io;

use super::state::uwriter::DirUWriterState;
use crate::bucket::{errors, Bucket};
use crate::entry::BorrowedEntry;
use crate::hasher::collector::BufCollector;
use crate::hasher::dir_hasher::DirectoryHasher;
use crate::stream::verifier::{IncrementalVerifier, WithHashTreeCollector};

pub struct UntrustedDirWriter {
    state: DirUWriterState,
}

impl UntrustedDirWriter {
    pub async fn new(
        bucket: &Bucket,
        num_entries: usize,
        hash: [u8; 32],
    ) -> Result<Self, errors::WriteError> {
        let uwriter = DirUWriterState::new(bucket, num_entries, hash)
            .await
            .map(|state| Self { state })?;
        Ok(uwriter)
    }

    pub async fn feed_proof(&mut self, proof: &[u8]) -> Result<(), errors::FeedProofError> {
        self.state.feed_proof(proof)
    }

    pub async fn insert<'a>(
        &mut self,
        entry: impl Into<BorrowedEntry<'a>>,
    ) -> Result<(), errors::InsertError> {
        self.state.insert_entry(entry).await
    }

    /// Finalize this write and flush the data to the disk.
    pub async fn commit(self) -> Result<[u8; 32], errors::CommitError> {
        self.state.commit().await
    }

    /// Cancel this write and remove anything that this writer wrote to the disk.
    pub async fn rollback(self) -> Result<(), io::Error> {
        self.state.rollback().await
    }
}
