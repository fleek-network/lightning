use std::io::{self, Write};
use std::num::NonZeroU32;
use std::sync::Arc;

use bytes::BytesMut;
use tokio::sync::RwLock;

use super::phf::PhfGenerator;
use super::state::writer::DirWriterState;
use crate::bucket::{errors, Bucket};
use crate::entry::{BorrowedEntry, BorrowedLink};
use crate::hasher::byte_hasher::Blake3Hasher;
use crate::hasher::collector::BufCollector;
use crate::hasher::HashTreeCollector as _;
use crate::stream::verifier::{IncrementalVerifier, WithHashTreeCollector};

/// A trusted directory writer.
pub struct DirWriter {
    state: DirWriterState,
}

impl DirWriter {
    pub async fn new(bucket: &Bucket, num_entries: usize) -> Result<Self, errors::WriteError> {
        let inner_state = DirWriterState::new(bucket, num_entries).await?;
        Ok(Self { state: inner_state })
    }

    pub async fn insert<'b>(
        &mut self,
        entry: impl Into<BorrowedEntry<'b>>,
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
