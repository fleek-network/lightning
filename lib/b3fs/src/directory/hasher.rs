use std::fmt::Debug;
use std::io::Write;

use arrayvec::ArrayVec;
use fleek_blake3::platform::Platform;

use super::constants::EMPTY_HASH;
use super::entry::{BorrowedEntry, BorrowedLink};
use super::error::Error;
use super::merge::merge;
use super::transcript::{hash_transcript, write_entry_transcript};
use crate::collections::HashTree;
use crate::directory::constants::IS_ROOT_FLAG;
use crate::utils::{flatten, is_valid_filename, is_valid_symlink, Digest};

/// A hashing algorithm based and derived from blake3 used for hashing directories.
#[derive(Clone, Debug)]
pub struct DirectoryHasher {
    platform: Platform,
    /// The chaining value stack for the hashing logic.
    stack: ArrayVec<[u8; 32], 48>,
    /// The current counter.
    counter: usize,
    /// The buffer we use to write the transcript of each entry upon insertion.
    transcript_buffer: Vec<u8>,
    /// The length of the previous file name. Because of the hashing delay on the actual
    /// transcript this means we can still find the filename in the transcript xD
    last_filename_size: usize,
    /// Is `Some` if we are capturing the tree.
    tree: Option<Vec<[u8; 32]>>,
}

pub struct HashDirectoryOutput {
    pub hash: [u8; 32],
    pub tree: Option<Vec<[u8; 32]>>,
}

impl DirectoryHasher {
    /// Create a new directory hasher, if the `capture_tree` parameters is set to `true` the hash
    /// tree will be captured and preserved.
    pub fn new(capture_tree: bool) -> Self {
        Self {
            platform: Platform::detect(),
            stack: ArrayVec::new(),
            counter: 0,
            transcript_buffer: Vec::with_capacity(320),
            last_filename_size: 0,
            tree: capture_tree.then(Vec::new),
        }
    }

    /// Reserve space for `n` more additional entries.
    pub fn reserve(&mut self, n: usize) {
        if n == 0 {
            return;
        }

        if let Some(tree) = &mut self.tree {
            let total_tree_size = (self.counter + n) * 2 - 1;
            let additional = total_tree_size - tree.len();
            tree.reserve(additional);
        }
    }

    /// Flush the captured tree to the given writer, on sucess removes all elements from its own
    /// buffer.
    ///
    /// # Panics
    ///
    /// If the hasher is not capturing the tree.
    pub fn flush_tree<W>(&mut self, writer: &mut W) -> std::io::Result<()>
    where
        W: Write,
    {
        let tree = self.tree.as_mut().unwrap();
        let flat_tree = flatten(&tree);
        match writer.write_all(flat_tree) {
            Ok(_) => {
                tree.clear();
                Ok(())
            },
            Err(e) => Err(e),
        }
    }

    /// Finalize the tree and write the pending tree nodes into the given writer.
    /// # Panics
    ///
    /// If the hasher is not capturing the tree.
    pub fn finalize_flush_tree<W>(self, writer: &mut W) -> std::io::Result<()>
    where
        W: Write,
    {
        let output = self.finalize();
        let tree = output.tree.unwrap();
        let flat_tree = flatten(&tree);
        writer.write_all(flat_tree)
    }

    /// Insert the given entry into the hasher and returns `Ok(())` if the entry passes all the
    /// sanity checks. Otherwise an error is returned, if an error is returned the state of the
    /// hasher is *not* corruputed and you can continue feed other entries. Just the entry that
    /// was provided here will not be part of that directory.
    pub fn insert<'b>(&mut self, entry: BorrowedEntry<'b>) -> Result<(), Error> {
        self.insert_inner::<false>(entry)
    }

    /// Insert the given entry into the hasher wihtout performing any of the sanity checks on the
    /// provided data. UB if the entry is not valid.
    pub fn insert_unchecked<'b>(&mut self, entry: BorrowedEntry<'b>) {
        self.insert_inner::<true>(entry).unwrap()
    }

    #[inline(always)]
    fn insert_inner<'b, const UNCHECKED: bool>(
        &mut self,
        entry: BorrowedEntry<'b>,
    ) -> Result<(), Error> {
        if !UNCHECKED {
            if self.counter == (u16::MAX as usize) {
                return Err(Error::TooManyEntries);
            }

            // Step 1: Check the validity of the file name against the previously provided
            // file name.
            if self.counter > 0 {
                let end = 3 + self.last_filename_size;
                let prev_name = &self.transcript_buffer[3..end];

                if prev_name == entry.name {
                    return Err(Error::FilenameNotUnique);
                }

                if prev_name > entry.name {
                    return Err(Error::InvalidOrdering);
                }
            }

            // Step 2: Check validity of variable content (i.e filename and link content)
            if !is_valid_filename(entry.name) {
                println!("{:?}", entry.name);
                return Err(Error::InvalidFileName);
            }

            if let BorrowedLink::Link(link) = entry.link {
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
    pub fn finalize(mut self) -> HashDirectoryOutput {
        // Return the hash of the empty directory in the special case.
        if self.counter == 0 {
            return HashDirectoryOutput::empty(self.tree.is_some());
        }

        // In case we have nothing in the stack turn on the `is_root` flag of the
        // current pending transcript and then hash it and push it to the stack.
        if self.stack.is_empty() {
            self.transcript_buffer[0] |= IS_ROOT_FLAG;
        }
        let cv = hash_transcript(&self.transcript_buffer);
        self.push_cv(cv);

        while self.stack.len() > 1 {
            let right_cv = self.stack.pop().unwrap();
            let left_cv = self.stack.pop().unwrap();
            let is_root = self.stack.is_empty();
            let parent = merge(self.platform, &left_cv, &right_cv, is_root);
            self.push_cv(parent);
        }

        let hash = self.stack.pop().unwrap();
        HashDirectoryOutput {
            hash,
            tree: self.tree,
        }
    }

    /// Push a new chaining value to the stack and the tree if we're capturing the tree.
    #[inline(always)]
    fn push_cv(&mut self, cv: [u8; 32]) {
        self.stack.push(cv);
        if let Some(tree) = &mut self.tree {
            tree.push(cv);
        }
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
            let parent = merge(self.platform, &left_cv, &right_cv, false);
            self.push_cv(parent);
        }
    }
}

impl HashDirectoryOutput {
    fn empty(capture_tree: bool) -> Self {
        Self {
            hash: EMPTY_HASH,
            tree: capture_tree.then(|| vec![EMPTY_HASH]),
        }
    }
}

impl Debug for HashDirectoryOutput {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HashDirectoryOutput")
            .field("hash", &Digest(&self.hash))
            .field(
                "tree",
                &self
                    .tree
                    .as_ref()
                    .map(|vec| HashTree::try_from(vec.as_slice()).unwrap()),
            )
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::directory::constants::EMPTY_HASH;
    use crate::test_utils::*;

    #[test]
    fn empty_hash() {
        let hasher = DirectoryHasher::new(false);
        let out = hasher.finalize();
        assert_eq!(out.hash, EMPTY_HASH);
        assert_eq!(out.tree, None);
        let hasher = DirectoryHasher::new(true);
        let out = hasher.finalize();
        assert_eq!(out.hash, EMPTY_HASH);
        assert_eq!(out.tree, Some(vec![EMPTY_HASH]));
    }

    #[test]
    fn one_item() {
        let mut hasher = DirectoryHasher::new(true);
        let e = entry(0);
        hasher.insert(BorrowedEntry::from(&e)).unwrap();
        let out = hasher.finalize();
        let expected_hash = hash_mock_dir_entry(0, true);
        assert_eq!(expected_hash, out.hash);
    }

    #[test]
    fn two_item() {
        let mut hasher = DirectoryHasher::new(true);
        let e = entry(0);
        hasher.insert(BorrowedEntry::from(&e)).unwrap();
        let e = entry(1);
        hasher.insert(BorrowedEntry::from(&e)).unwrap();

        let out = hasher.finalize();

        let expected_hash = merge(
            Platform::detect(),
            &hash_mock_dir_entry(0, false),
            &hash_mock_dir_entry(1, false),
            true,
        );

        assert_eq!(expected_hash, out.hash);
    }
}
