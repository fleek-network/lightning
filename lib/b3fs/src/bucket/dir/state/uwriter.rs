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
use crate::bucket::{errors, Bucket};
use crate::entry::{BorrowedEntry, BorrowedLink};
use crate::hasher::b3::MAX_BLOCK_SIZE_IN_BYTES;
use crate::hasher::collector::BufCollector;
use crate::hasher::dir_hasher::DirectoryHasher;
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
}

impl DirUWriterCollector {
    /// Creates a new DirUWriterCollector with the given root hash
    pub(crate) fn new(hash: [u8; 32]) -> Self {
        let mut hasher = Box::new(IncrementalVerifier::<WithHashTreeCollector>::dir());
        hasher.set_root_hash(hash);
        Self {
            hasher,
            root_hash: hash,
        }
    }

    /// Feeds a proof to the incremental verifier
    pub(crate) fn feed_proof(&mut self, proof: &[u8]) -> Result<(), errors::FeedProofError> {
        self.hasher.feed_proof(proof).map_err(Into::into)
    }
}

impl WithCollector for DirUWriterCollector {
    /// Called when an entry is inserted into the directory
    /// Verifies that the entry hash matches the expected hash in the proof chain
    async fn on_insert(
        &mut self,
        borrowed_entry: BorrowedEntry<'_>,
    ) -> Result<(), errors::InsertError> {
        let mut dir_hasher: DirectoryHasher<Vec<[u8; 32]>> = DirectoryHasher::default();
        dir_hasher.insert(borrowed_entry);

        let (_, tree) = dir_hasher.finalize();

        dbg!(&tree);
        let this_entry_hash = tree
            .get(tree_index(0))
            .ok_or(errors::InsertError::IncrementalVerification)?;

        dbg!(this_entry_hash);

        self.hasher
            .verify_hash(*this_entry_hash)
            .map_err(|_| errors::InsertError::IncrementalVerification)
    }

    /// Called when committing the directory changes
    /// Returns the root hash and final hash tree
    async fn on_commit(self) -> Result<([u8; 32], Vec<[u8; 32]>), errors::CommitError> {
        Ok((self.root_hash, self.hasher.finalize()))
    }
}

/// Type alias for the directory update writer state using DirUWriterCollector
pub type DirUWriterState = InnerDirState<DirUWriterCollector>;
