//! This module contains the implementation of the directory hashing algorithm.

use fleek_blake3::platform::{self, Platform};

use super::{Digest, DirectoryEntry};
use crate::utils::{HashTree, HashVec};

// Some of the flags same as blake3 spec.
const ROOT: u8 = 1 << 3;
const PARENT: u8 = 1 << 2;
const KEYED_HASH: u8 = 1 << 4;

/// Blake3 hash of the word `"DIRECTORY"` used as the key for the hashing.
pub const KEY: [u8; 32] = [
    139, 88, 112, 131, 96, 138, 152, 197, 238, 63, 142, 210, 224, 88, 97, 183, 244, 210, 116, 213,
    84, 215, 9, 16, 21, 175, 61, 72, 251, 174, 76, 21,
];

/// Hash of an empty directory which is set to `KeyedHash(KEY, &[])`.
pub const EMPTY_HASH: [u8; 32] = [
    72, 63, 122, 133, 174, 60, 219, 10, 52, 209, 178, 47, 200, 109, 164, 116, 12, 53, 178, 104,
    128, 89, 147, 234, 130, 71, 29, 80, 131, 193, 231, 128,
];

pub struct HashDirectoryOutput {
    pub hash: fleek_blake3::Hash,
    pub tree: Option<HashTree>,
}

/// Hash a directory as indicated by a list of directory entries. To get a consistent hash
/// for the same directory the provided array must already be sorted.
pub fn hash_directory(collect_tree: bool, entries: &[DirectoryEntry]) -> HashDirectoryOutput {
    // Only to group stack and tree.
    struct State {
        stack: arrayvec::ArrayVec<Digest, 47>,
        tree: Option<Vec<u8>>,
    }

    impl State {
        #[inline(always)]
        pub fn push(&mut self, hash: Digest) {
            self.stack.push(hash);
            if let Some(tree) = &mut self.tree {
                tree.extend_from_slice(&hash);
            }
        }

        #[inline(always)]
        pub fn pop(&mut self) -> Digest {
            self.stack.pop().unwrap()
        }
    }

    if entries.is_empty() {
        return HashDirectoryOutput {
            hash: fleek_blake3::Hash::from_bytes(EMPTY_HASH),
            // TODO(qti3e): Verify if this beahvious is consistent with the
            // `fleek_blake3::HashTreeBuilder`.
            tree: collect_tree.then(|| HashTree::from(&[EMPTY_HASH] as &[[u8; 32]])),
        };
    }

    // TODO(qti3e): This can be optimized by using `Platform::hash_many`.

    let platform = Platform::detect();
    let mut buffer = Vec::with_capacity(512);
    let mut counter = 0;
    let mut container = State {
        stack: arrayvec::ArrayVec::<Digest, 47>::new(),
        tree: collect_tree.then(|| Vec::<u8>::with_capacity(32 * (entries.len() * 2 - 1))),
    };

    let take = entries.len() - 1;

    while counter < take {
        entries[counter].transcript(&mut buffer, counter, false);
        let digest = *fleek_blake3::keyed_hash(&KEY, &buffer).as_bytes();
        buffer.clear();

        container.push(digest);
        counter += 1;

        let mut total_entries = counter;
        while (total_entries & 1) == 0 {
            let right_cv = container.pop();
            let left_cv = container.pop();
            let parent_cv = merge(platform, &left_cv, &right_cv, false);
            container.push(parent_cv);
            total_entries >>= 1;
        }
    }

    // Handle the last entry, which might be the only entry in which case we
    // need to pass IS_ROOT to it.
    let is_root = container.stack.is_empty();
    entries[counter].transcript(&mut buffer, counter, is_root);
    let digest = *fleek_blake3::keyed_hash(&KEY, &buffer).as_bytes();
    container.push(digest);

    // Merge the stack from right to left and set the finalize flag of the last element.
    while container.stack.len() > 1 {
        let right_cv = container.pop();
        let left_cv = container.pop();
        let is_root = container.stack.is_empty();
        let parent = merge(platform, &left_cv, &right_cv, is_root);
        container.push(parent);
    }

    let hash = fleek_blake3::Hash::from_bytes(container.pop());
    let tree = container
        .tree
        .map(|tree| HashTree::from_inner(HashVec::try_from(tree).unwrap()));

    HashDirectoryOutput { hash, tree }
}

#[inline(always)]
fn merge(platform: Platform, left_cv: &[u8; 32], right_cv: &[u8; 32], is_root: bool) -> [u8; 32] {
    let mut block = [0; 64];
    block[..32].copy_from_slice(left_cv);
    block[32..].copy_from_slice(right_cv);
    let mut cv = platform::words_from_le_bytes_32(&KEY);
    platform.compress_in_place(
        &mut cv,
        &block,
        64,
        0, // counter is always zero for parents.
        if is_root {
            PARENT | KEYED_HASH | ROOT
        } else {
            PARENT | KEYED_HASH
        },
    );
    platform::le_bytes_from_words_32(&cv)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::directory::Link;

    #[test]
    fn constants() {
        let key = *fleek_blake3::hash("DIRECTORY".as_bytes()).as_bytes();
        assert_eq!(key, KEY);
        let empty_hash = *fleek_blake3::keyed_hash(&key, &[]).as_bytes();
        assert_eq!(empty_hash, EMPTY_HASH);
    }

    fn hash_entry(is_root: bool, counter: usize, entry: &DirectoryEntry) -> [u8; 32] {
        let mut buffer = Vec::with_capacity(128);
        entry.transcript(&mut buffer, counter, is_root);
        *fleek_blake3::keyed_hash(&KEY, &buffer).as_bytes()
    }

    #[test]
    fn test() {
        let dirs = vec![
            DirectoryEntry::new("A".into(), Link::file([0; 32])),
            DirectoryEntry::new("B".into(), Link::file([1; 32])),
            DirectoryEntry::new("C".into(), Link::file([2; 32])),
            DirectoryEntry::new("D".into(), Link::file([3; 32])),
            DirectoryEntry::new("E".into(), Link::file([4; 32])),
            DirectoryEntry::new("F".into(), Link::file([5; 32])),
            DirectoryEntry::new("G".into(), Link::file([6; 32])),
            DirectoryEntry::new("H".into(), Link::file([7; 32])),
            DirectoryEntry::new("I".into(), Link::file([8; 32])),
            DirectoryEntry::new("J".into(), Link::file([9; 32])),
        ];

        let platform = Platform::detect();

        let output = hash_directory(false, &dirs[0..1]);
        let actual = *output.hash.as_bytes();
        let expected = hash_entry(true, 0, &dirs[0]);
        assert_eq!(actual, expected);

        let output = hash_directory(false, &dirs[0..2]);
        let actual = *output.hash.as_bytes();
        let expected = merge(
            platform,
            &hash_entry(false, 0, &dirs[0]),
            &hash_entry(false, 1, &dirs[1]),
            true,
        );
        assert_eq!(actual, expected);

        let output = hash_directory(false, &dirs[0..3]);
        let actual = *output.hash.as_bytes();
        let expected = merge(
            platform,
            &merge(
                platform,
                &hash_entry(false, 0, &dirs[0]),
                &hash_entry(false, 1, &dirs[1]),
                false,
            ),
            &hash_entry(false, 2, &dirs[2]),
            true,
        );
        assert_eq!(actual, expected);

        let output = hash_directory(false, &dirs[0..4]);
        let actual = *output.hash.as_bytes();
        let expected = merge(
            platform,
            &merge(
                platform,
                &hash_entry(false, 0, &dirs[0]),
                &hash_entry(false, 1, &dirs[1]),
                false,
            ),
            &merge(
                platform,
                &hash_entry(false, 2, &dirs[2]),
                &hash_entry(false, 3, &dirs[3]),
                false,
            ),
            true,
        );
        assert_eq!(actual, expected);

        let output = hash_directory(false, &dirs[0..5]);
        let actual = *output.hash.as_bytes();
        let expected = merge(
            platform,
            &merge(
                platform,
                &merge(
                    platform,
                    &hash_entry(false, 0, &dirs[0]),
                    &hash_entry(false, 1, &dirs[1]),
                    false,
                ),
                &merge(
                    platform,
                    &hash_entry(false, 2, &dirs[2]),
                    &hash_entry(false, 3, &dirs[3]),
                    false,
                ),
                false,
            ),
            &hash_entry(false, 4, &dirs[4]),
            true,
        );
        assert_eq!(actual, expected);
    }
}
