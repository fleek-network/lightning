//! Untrusted directory writer implementation.
//!
//! This module provides functionality for writing directory entries in an untrusted manner,
//! requiring proofs to verify the integrity of the data being written.

use std::fmt::Debug;
use std::io;

use super::state::uwriter::{DirUWriterCollector, DirUWriterState};
use super::state::{DirState, InnerDirState};
use crate::entry::BorrowedEntry;
use crate::hasher::collector::BufCollector;
use crate::hasher::dir_hasher::DirectoryHasher;
use crate::stream::verifier::{IncrementalVerifier, WithHashTreeCollector};
use crate::sync::bucket::{errors, Bucket};

/// A writer for untrusted directory operations that requires proofs for verification.
pub struct UntrustedDirWriter {
    /// The internal state of the writer
    state: DirUWriterState,
}

impl Debug for UntrustedDirWriter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("UntrustedDirWriter").finish()
    }
}

impl UntrustedDirWriter {
    /// Creates a new untrusted directory writer.
    ///
    /// # Arguments
    /// * `bucket` - The bucket to write to
    /// * `num_entries` - Number of entries that will be written
    /// * `hash` - Expected root hash for verification
    ///
    /// # Returns
    /// A new UntrustedDirWriter instance or a WriteError
    pub fn new(
        bucket: &Bucket,
        num_entries: usize,
        hash: [u8; 32],
    ) -> Result<Self, errors::WriteError> {
        let collector = DirUWriterCollector::new(hash);
        let state = InnerDirState::new_with_collector(bucket, num_entries, collector)
            .map(|state| Self { state })?;
        Ok(state)
    }

    /// Feeds a proof to the writer for verification.
    ///
    /// # Arguments
    /// * `proof` - The proof bytes to verify
    ///
    /// # Returns
    /// Ok(()) if the proof is valid, FeedProofError otherwise
    pub fn feed_proof(&mut self, proof: &[u8]) -> Result<(), errors::FeedProofError> {
        self.state.collector.feed_proof(proof)
    }

    /// Inserts an entry into the directory.
    ///
    /// # Arguments
    /// * `entry` - The entry to insert
    ///
    /// # Returns
    /// Ok(()) if successful, InsertError otherwise
    pub fn insert<'a>(
        &mut self,
        entry: impl Into<BorrowedEntry<'a>>,
        last_entry: bool,
    ) -> Result<(), errors::InsertError> {
        self.state.insert_entry(entry.into(), last_entry)
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

    use core::num;

    use super::*;
    use crate::collections::HashTree;
    use crate::entry::{OwnedEntry, OwnedLink};
    use crate::hasher::byte_hasher::Blake3Hasher;
    use crate::hasher::dir_hasher::DirectoryHasher;
    use crate::stream::walker::Mode;
    use crate::sync::bucket::dir::tests::setup_bucket;
    use crate::sync::bucket::tests::get_random_file;
    use crate::sync::bucket::Bucket;
    use crate::test_utils::*;

    fn setup_hasher(num_entries: usize) -> (Vec<[u8; 32]>, [u8; 32], Vec<OwnedEntry>) {
        let mut dir_hasher = DirectoryHasher::default();
        let mut entries = vec![];
        for i in 0..num_entries {
            let mut hasher: Blake3Hasher<Vec<[u8; 32]>> = Blake3Hasher::default();
            let block = get_random_file(8192 * 2);
            hasher.update(&block[..]);

            let (ref mut hashes, root) = hasher.finalize_tree();
            hashes.push(root);
            let entry = OwnedEntry {
                name: format!("test_file_{i}.txt").as_bytes().into(),
                link: OwnedLink::Content(root),
            };
            let borrowed = BorrowedEntry::from(&entry);
            dir_hasher.insert_unchecked(borrowed);
            entries.push(entry);
        }
        let (root, tree) = dir_hasher.finalize();
        (tree, root, entries)
    }

    #[test]
    fn test_untrusted_dir_writer_insert() {
        let bucket = setup_bucket().unwrap();
        let num_entries = 1;
        let (hashes, root, entry) = setup_hasher(num_entries);
        let hashtree = HashTree::try_from(&hashes).unwrap();

        let mut writer = UntrustedDirWriter::new(&bucket, num_entries, root).unwrap();

        writer
            .feed_proof(
                hashtree
                    .generate_proof(Mode::from_is_initial(true), 0)
                    .as_slice(),
            )
            .unwrap();
        let result = writer.insert(entry.first().unwrap(), true);
        assert!(result.is_ok());
    }

    #[test]
    fn test_untrusted_dir_writer_commit() {
        let bucket = setup_bucket().unwrap();
        let num_entries = 1;
        let (hashes, root, entry) = setup_hasher(num_entries);
        let hashtree = HashTree::try_from(&hashes).unwrap();

        let mut writer = UntrustedDirWriter::new(&bucket, num_entries, root).unwrap();

        let proof = hashtree.generate_proof(Mode::from_is_initial(true), 0);

        writer.feed_proof(proof.as_slice()).unwrap();

        writer.insert(entry.first().unwrap(), true).unwrap();

        let result = writer.commit();
        assert!(result.is_ok());
        assert_eq!(result.unwrap().len(), 32);
    }

    #[test]
    fn test_untrusted_dir_writer_rollback() {
        let bucket = setup_bucket().unwrap();
        let num_entries = 1;
        let (hashes, root, entry) = setup_hasher(num_entries);
        let hashtree = HashTree::try_from(&hashes).unwrap();

        let mut writer = UntrustedDirWriter::new(&bucket, num_entries, root).unwrap();

        writer
            .feed_proof(
                hashtree
                    .generate_proof(Mode::from_is_initial(true), 0)
                    .as_slice(),
            )
            .unwrap();

        writer.insert(entry.first().unwrap(), true).unwrap();

        let result = writer.rollback();
        assert!(result.is_ok());
    }

    #[test]
    fn test_untrusted_dir_writer_several_entries() {
        let bucket = setup_bucket().unwrap();
        let num_entries = 3;
        let (hashes, root, entries) = setup_hasher(num_entries);
        let hashtree = HashTree::try_from(&hashes).unwrap();

        let mut writer = UntrustedDirWriter::new(&bucket, num_entries, root).unwrap();

        for (i, entry) in entries.iter().enumerate() {
            let generate_proof = hashtree.generate_proof(Mode::from_is_initial(i == 0), i);
            writer.feed_proof(generate_proof.as_slice()).unwrap();

            writer.insert(entry, i + 1 == num_entries).unwrap();
        }
        let result = writer.commit();
        assert!(result.is_ok());
        assert_eq!(result.unwrap().len(), 32);
    }
}
