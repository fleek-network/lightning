use std::io;

use super::state::uwriter::{DirUWriterCollector, DirUWriterState};
use super::state::{DirState, InnerDirState};
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
        let collector = DirUWriterCollector::new(hash);
        let state = InnerDirState::new_with_collector(bucket, num_entries, collector)
            .await
            .map(|state| Self { state })?;
        Ok(state)
    }

    pub async fn feed_proof(&mut self, proof: &[u8]) -> Result<(), errors::FeedProofError> {
        self.state.collector.feed_proof(proof)
    }

    pub async fn insert<'a>(
        &mut self,
        entry: impl Into<BorrowedEntry<'a>>,
    ) -> Result<(), errors::InsertError> {
        self.state.insert_entry(entry.into()).await
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
