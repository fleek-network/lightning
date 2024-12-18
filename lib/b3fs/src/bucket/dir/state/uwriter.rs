use std::num::NonZeroU32;
use std::os::unix::fs::FileExt;
use std::path::{Path, PathBuf};

use bytes::BytesMut;
use fastbloom_rs::{FilterBuilder, Membership};
use rand::random;
use serde::Serialize as _;
use tokio::fs::{File, OpenOptions};
use tokio::io::{self, AsyncWriteExt, BufWriter};

use super::*;
use crate::bucket::dir::phf::{HasherState, PhfGenerator};
use crate::bucket::errors::CommitError;
use crate::bucket::{errors, Bucket};
use crate::entry::{BorrowedEntry, BorrowedLink, OwnedEntry};
use crate::hasher::b3::MAX_BLOCK_SIZE_IN_BYTES;
use crate::hasher::collector::BufCollector;
use crate::hasher::dir_hasher::{write_entry_transcript, DirectoryHasher};
use crate::hasher::iv::IV;
use crate::hasher::HashTreeCollector;
use crate::stream::verifier::{IncrementalVerifier, WithHashTreeCollector};
use crate::utils::{self, tree_index};

/// Collector for incrementally verifying and building a directory hash tree during updates
#[derive(Default)]
pub(crate) struct DirUWriterCollector {
    /// The incremental verifier with hash tree collection capability
    hasher: Box<IncrementalVerifier<WithHashTreeCollector>>,
    /// The expected root hash of the directory
    root_hash: [u8; 32],

    counter: u16,

    last_entry: Option<OwnedEntry>,
}

impl DirUWriterCollector {
    /// Creates a new DirUWriterCollector with the given root hash
    pub(crate) fn new(hash: [u8; 32]) -> Self {
        let mut hasher = Box::new(IncrementalVerifier::<WithHashTreeCollector>::dir());
        hasher.set_root_hash(hash);
        Self {
            hasher,
            root_hash: hash,
            last_entry: None,
            counter: 0,
        }
    }

    /// Feeds a proof to the incremental verifier
    pub(crate) fn feed_proof(&mut self, proof: &[u8]) -> Result<(), errors::FeedProofError> {
        self.hasher.feed_proof(proof).map_err(Into::into)
    }

    fn create_hash(&mut self, entry: BorrowedEntry<'_>, last_entry: bool) -> [u8; 32] {
        let mut buffer = Vec::new();
        write_entry_transcript(
            &mut buffer,
            entry,
            self.counter,
            last_entry && self.counter == 0,
        );
        let hash = IV::DIR.hash_all_at_once(&buffer);
        self.counter += 1;
        hash
    }
}

impl WithCollector for DirUWriterCollector {
    /// Called when an entry is inserted into the directory
    /// Verifies that the entry hash matches the expected hash in the proof chain
    async fn on_insert(
        &mut self,
        borrowed_entry: BorrowedEntry<'_>,
        last_entry: bool,
    ) -> Result<(), errors::InsertError> {
        if last_entry {
            self.last_entry = Some(borrowed_entry.into());
            return Ok(());
        }

        let hash = self.create_hash(borrowed_entry, last_entry);

        self.hasher
            .verify_hash(hash)
            .map_err(|e| errors::InsertError::IncrementalVerification(e.to_string()))
    }

    /// Called when committing the directory changes
    /// Returns the root hash and final hash tree
    async fn on_commit(mut self) -> Result<([u8; 32], Vec<[u8; 32]>), errors::CommitError> {
        if let Some(ref mut last_entry) = self.last_entry {
            let owned_entry = last_entry.to_owned();
            let borrowed = BorrowedEntry::from(&owned_entry);
            let hash = self.create_hash(borrowed, true);
            self.hasher
                .verify_hash(hash)
                .map_err(|e| errors::InsertError::IncrementalVerification(e.to_string()))?;
        }
        Ok((self.root_hash, self.hasher.finalize()))
    }
}

/// Type alias for the directory update writer state using DirUWriterCollector
pub type DirUWriterState = InnerDirState<DirUWriterCollector>;
