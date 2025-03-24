use std::fmt::Debug;
use std::io::Write;

use super::b3::platform::Platform;
use super::iv::{SmallIV, IV};
use super::HashTreeCollector;
#[cfg(not(feature = "sync"))]
use crate::collections::HashTree;
use crate::entry::{BorrowedEntry, BorrowedLink};
use crate::utils::{flatten, is_valid_filename, is_valid_symlink, Digest};

mod error;
mod reference;
mod transcript;

use arrayvec::ArrayVec;
pub use error::*;
pub use transcript::*;

/// A hashing algorithm based and derived from blake3 used for hashing directories.
#[derive(Clone, Debug)]
pub struct DirectoryHasher<T: HashTreeCollector = Vec<[u8; 32]>> {
    platform: Platform,
    /// The chaining value stack for the hashing logic.
    stack: ArrayVec<[u8; 32], 17>,
    /// The current counter.
    counter: usize,
    /// The buffer we use to write the transcript of each entry upon insertion.
    transcript_buffer: Vec<u8>,
    /// The length of the previous file name. Because of the hashing delay on the actual
    /// transcript this means we can still find the filename in the transcript xD
    last_filename_size: usize,
    /// The tree collector.
    tree: T,
}

impl<T: HashTreeCollector> DirectoryHasher<T> {
    /// Create a new directory hasher, if the `capture_tree` parameters is set to `true` the hash
    /// tree will be captured and preserved.
    pub fn new(tree: T) -> Self {
        Self {
            platform: Platform::detect(),
            stack: ArrayVec::new(),
            counter: 0,
            transcript_buffer: Vec::with_capacity(320),
            last_filename_size: 0,
            tree,
        }
    }

    /// Reserve space for `n` more additional entries.
    pub fn reserve(&mut self, n: usize) {
        if n == 0 {
            return;
        }

        let total_tree_size = (self.counter + n) * 2 - 1;
        let additional = total_tree_size - self.tree.len();
        self.tree.reserve(additional);
    }

    /// Insert the given entry into the hasher and returns `Ok(())` if the entry passes all the
    /// sanity checks. Otherwise an error is returned, if an error is returned the state of the
    /// hasher is *not* corruputed and you can continue feed other entries. Just the entry that
    /// was provided here will not be part of that directory.
    pub fn insert(&mut self, entry: BorrowedEntry) -> Result<(), Error> {
        self.insert_inner::<false>(entry)
    }

    /// Insert the given entry into the hasher wihtout performing any of the sanity checks on the
    /// provided data. UB if the entry is not valid.
    pub fn insert_unchecked(&mut self, entry: BorrowedEntry) {
        self.insert_inner::<true>(entry).unwrap()
    }

    #[inline(always)]
    fn insert_inner<const UNCHECKED: bool>(&mut self, entry: BorrowedEntry) -> Result<(), Error> {
        if !UNCHECKED {
            if self.counter == (u16::MAX as usize) {
                return Err(Error::TooManyEntries);
            }

            // Step 1: Check the validity of the file name against the previously provided
            // file name.
            if self.counter > 0 {
                let end = 3 + self.last_filename_size;
                let prev_name = &self.transcript_buffer[1..end];

                if prev_name == entry.name {
                    return Err(Error::FilenameNotUnique);
                }

                if prev_name > entry.name {
                    return Err(Error::InvalidOrdering);
                }
            }

            // Step 2: Check validity of variable content (i.e filename and link content)
            if !is_valid_filename(entry.name) {
                return Err(Error::InvalidFileName);
            }

            if let BorrowedLink::Path(link) = entry.link {
                if !is_valid_symlink(link) {
                    return Err(Error::InvalidSymlink);
                }
            }
        }

        // Finalize the previous transcript buffer.
        if self.counter > 0 {
            let cv = hash_transcript(&self.transcript_buffer);
            self.push_cv(cv);
            self.merge_cv_stack();
        }

        // Now reuse the buffer and move the counter forward.
        self.transcript_buffer.clear();
        write_entry_transcript(
            &mut self.transcript_buffer,
            entry,
            self.counter as u16,
            false,
        );
        self.last_filename_size = entry.name.len();
        self.counter += 1;

        Ok(())
    }

    /// Finalize the hashing process and returns the output of the hashing algorithm. The
    /// generated tree is also returned if the hasher was instantiated with the capture mode
    /// enabled.
    pub fn finalize(mut self) -> ([u8; 32], T) {
        // Return the hash of the empty directory in the special case.
        if self.counter == 0 {
            let empty = *IV::DIR.empty_hash();
            self.tree.push(empty);
            return (empty, self.tree);
        }

        // In case we have nothing in the stack turn on the `is_root` flag of the
        // current pending transcript and then hash it and push it to the stack.
        if self.stack.is_empty() {
            self.transcript_buffer[0] |= B3_DIR_IS_ROOT;
        }

        let cv = hash_transcript(&self.transcript_buffer);
        self.push_cv(cv);

        while self.stack.len() > 1 {
            let right_cv = self.stack.pop().unwrap();
            let left_cv = self.stack.pop().unwrap();
            let is_root = self.stack.is_empty();
            let parent = IV::DIR.merge(&left_cv, &right_cv, is_root);
            self.push_cv(parent);
        }

        let hash = self.stack.pop().unwrap();
        (hash, self.tree)
    }

    /// Push a new chaining value to the stack and the tree if we're capturing the tree.
    #[inline(always)]
    fn push_cv(&mut self, cv: [u8; 32]) {
        self.stack.push(cv);
        self.tree.push(cv);
    }

    #[inline(always)]
    fn merge_cv_stack(&mut self) {
        // no +1 here because we're already on the next counter due to our lazy finalization
        // of each transcript.
        let mut total_entries = self.counter;
        while (total_entries & 1) == 0 {
            total_entries >>= 1;
            let right_cv = self.stack.pop().unwrap();
            let left_cv = self.stack.pop().unwrap();
            let parent = IV::DIR.merge(&left_cv, &right_cv, false);
            self.push_cv(parent);
        }
    }
}

impl<T: HashTreeCollector> Default for DirectoryHasher<T>
where
    T: Default,
{
    fn default() -> Self {
        Self::new(T::default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hasher::SkipHashTreeCollector;
    use crate::test_utils::*;

    const EMPTY_HASH: [u8; 32] = *IV::DIR.empty_hash();

    #[test]
    fn empty_hash() {
        let hasher = DirectoryHasher::<SkipHashTreeCollector>::default();
        let (hash, tree) = hasher.finalize();
        assert_eq!(hash, EMPTY_HASH);
        assert_eq!(tree.len(), 1);

        let hasher = DirectoryHasher::<Vec<[u8; 32]>>::default();
        let (hash, tree) = hasher.finalize();
        assert_eq!(hash, EMPTY_HASH);
        assert_eq!(tree, vec![EMPTY_HASH]);
    }

    #[test]
    fn one_item() {
        let mut hasher = DirectoryHasher::<Vec<[u8; 32]>>::default();
        let e = entry(0);
        hasher.insert(BorrowedEntry::from(&e)).unwrap();
        let (hash, _) = hasher.finalize();
        let expected_hash = hash_mock_dir_entry(0, true);
        assert_eq!(expected_hash, hash);
    }

    #[test]
    fn two_item() {
        let mut hasher = DirectoryHasher::<Vec<[u8; 32]>>::default();
        let e = entry(0);
        hasher.insert(BorrowedEntry::from(&e)).unwrap();
        let e = entry(1);
        hasher.insert(BorrowedEntry::from(&e)).unwrap();

        let (hash, tree) = hasher.finalize();

        let expected_hash = IV::DIR.merge(
            &hash_mock_dir_entry(0, false),
            &hash_mock_dir_entry(1, false),
            true,
        );

        assert_eq!(expected_hash, hash);
        assert_eq!(
            tree,
            vec![
                hash_mock_dir_entry(0, false),
                hash_mock_dir_entry(1, false),
                expected_hash
            ]
        );
    }
}
