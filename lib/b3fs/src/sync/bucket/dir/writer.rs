//! Trusted directory writer implementation.
//!
//! This module provides functionality for writing directory entries in a trusted manner,
//! without requiring proofs for verification.

use std::io::{self, Write};
use std::num::NonZeroU32;
use std::sync::Arc;

use bytes::BytesMut;

use super::phf::PhfGenerator;
use super::state::writer::DirWriterState;
use super::state::{DirState as _, InnerDirState};
use crate::entry::{BorrowedEntry, BorrowedLink};
use crate::hasher::byte_hasher::Blake3Hasher;
use crate::hasher::collector::BufCollector;
use crate::hasher::HashTreeCollector as _;
use crate::stream::verifier::{IncrementalVerifier, WithHashTreeCollector};
use crate::sync::bucket::{errors, Bucket};

/// A trusted directory writer for writing entries to a B3 directory.
///
/// Unlike `UntrustedDirWriter`, this writer does not require proofs for verification
/// since it is used in trusted contexts.
pub struct DirWriter {
    /// The internal state of the writer
    state: DirWriterState,
}

impl DirWriter {
    /// Creates a new trusted directory writer.
    ///
    /// # Arguments
    /// * `bucket` - The bucket to write to
    /// * `num_entries` - Number of entries that will be written
    ///
    /// # Returns
    /// A new DirWriter instance or a WriteError
    pub fn new(bucket: &Bucket, num_entries: usize) -> Result<Self, errors::WriteError> {
        let state = InnerDirState::new(bucket, num_entries).map(|state| Self { state })?;
        Ok(state)
    }

    /// Inserts an entry into the directory.
    ///
    /// # Arguments
    /// * `entry` - The entry to insert
    ///
    /// # Returns
    /// Ok(()) if successful, InsertError otherwise
    pub fn insert<'b>(
        &mut self,
        entry: impl Into<BorrowedEntry<'b>>,
    ) -> Result<(), errors::InsertError> {
        self.state.insert_entry(entry.into(), false)
    }

    /// Finalizes this write and flushes the data to disk.
    ///
    /// # Returns
    /// The root hash of the written directory if successful, CommitError otherwise
    pub fn commit(self) -> Result<[u8; 32], errors::CommitError> {
        self.state.commit()
    }

    /// Cancels this write and removes any data written to disk.
    ///
    /// # Returns
    /// Ok(()) if successful, io::Error otherwise
    pub fn rollback(self) -> Result<(), io::Error> {
        self.state.rollback()
    }
}

#[cfg(test)]
mod tests {
    use std::env::temp_dir;

    use super::*;
    use crate::entry::{BorrowedEntry, BorrowedLink, OwnedEntry, OwnedLink};
    use crate::sync::bucket::dir::tests::setup_bucket;
    use crate::sync::bucket::Bucket;

    #[test]
    fn test_dir_writer_basic() -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = temp_dir().join("b3fs");
        let bucket = Bucket::open(&temp_dir)?;

        // Test new()
        let mut writer = DirWriter::new(&bucket, 2)?;

        // Test insert()
        let entry1 = OwnedEntry {
            name: "file1.txt".as_bytes().into(),
            link: OwnedLink::Content([1; 32]),
        };
        let entry2 = OwnedEntry {
            name: "file2.txt".as_bytes().into(),
            link: OwnedLink::Content([2; 32]),
        };
        writer.insert(&entry1)?;
        writer.insert(&entry2)?;

        // Test commit()
        let root_hash = writer.commit()?;
        assert_eq!(root_hash.len(), 32);

        // Test rollback()
        let mut writer = DirWriter::new(&bucket, 1)?;
        let entry3 = OwnedEntry {
            name: "file3.txt".as_bytes().into(),
            link: OwnedLink::Content([3; 32]),
        };
        writer.insert(&entry3)?;
        writer.rollback()?;

        Ok(())
    }

    #[test]
    fn test_max_entries() -> Result<(), Box<dyn std::error::Error>> {
        let bucket = setup_bucket()?;
        let mut writer = DirWriter::new(&bucket, 100)?;

        let mut file_names = (0..100)
            .map(|i| (i, format!("file{}.txt", i)))
            .collect::<Vec<_>>();

        file_names.sort_by(|a, b| a.1.cmp(&b.1));

        for (i, name) in file_names {
            let entry = OwnedEntry {
                name: name.as_bytes().into(),
                link: OwnedLink::Content([i as u8; 32]),
            };
            writer.insert(&entry)?;
        }

        let root_hash = writer.commit()?;
        assert_eq!(root_hash.len(), 32);
        Ok(())
    }
}
