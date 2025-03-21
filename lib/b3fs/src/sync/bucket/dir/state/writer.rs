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
use crate::entry::{BorrowedEntry, BorrowedLink};
use crate::hasher::b3::MAX_BLOCK_SIZE_IN_BYTES;
use crate::hasher::collector::BufCollector;
use crate::hasher::dir_hasher::DirectoryHasher;
use crate::hasher::HashTreeCollector;
use crate::sync::bucket::dir::phf::{HasherState, PhfGenerator};
use crate::sync::bucket::{errors, Bucket};
use crate::utils;

/// Collector for building a directory hash tree during writes
#[derive(Default)]
pub(crate) struct DirWriterCollector {
    /// The directory hasher used to build the hash tree
    hasher: DirectoryHasher,
}

impl WithCollector for DirWriterCollector {
    /// Called when an entry is inserted into the directory
    /// Adds the entry to the directory hasher
    fn on_insert(
        &mut self,
        borrowed_entry: BorrowedEntry<'_>,
        _last_entry: bool,
    ) -> Result<(), errors::InsertError> {
        self.hasher.insert(borrowed_entry)?;
        Ok(())
    }

    /// Called when committing the directory changes
    /// Returns the root hash and final hash tree
    fn on_commit(self) -> Result<([u8; 32], Vec<[u8; 32]>), errors::CommitError> {
        Ok(self.hasher.finalize())
    }
}

/// Type alias for the directory writer state using DirWriterCollector
pub type DirWriterState = InnerDirState<DirWriterCollector>;
