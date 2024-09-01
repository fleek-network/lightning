//! The reference implementation of the directory hashing algorithm used for testing purposes.

use super::Error;
use crate::entry::{BorrowedEntry, OwnedEntry, OwnedLink};
use crate::hasher::b3::IV;
use crate::hasher::dir_hasher::{hash_entry, hash_transcript, write_entry_transcript};
use crate::hasher::iv::IV;
use crate::hasher::join::{Join, RayonJoin, SerialJoin};
use crate::utils::{
    is_valid_filename,
    is_valid_symlink,
    largest_power_of_two_leq,
    previous_pow_of_two,
};

pub fn hash_dir(entries: &[OwnedEntry]) -> Result<[u8; 32], Error> {
    if entries.len() > (u16::MAX as usize) {
        return Err(Error::TooManyEntries);
    }

    let mut prev: &[u8] = &[];
    for (i, e) in entries.iter().enumerate() {
        if !is_valid_filename(&e.name) {
            return Err(Error::InvalidFileName);
        }

        if let OwnedLink::Link(link) = &e.link {
            if !is_valid_symlink(link) {
                return Err(Error::InvalidSymlink);
            }
        }

        if e.name.as_slice() == prev {
            return Err(Error::FilenameNotUnique);
        }

        if e.name.as_slice() < prev {
            return Err(Error::InvalidOrdering);
        }

        prev = e.name.as_slice();
    }

    // end of validations

    if entries.is_empty() {
        return Ok(IV::DIR.hash_all_at_once(&[]));
    }

    Ok(split_hash_merge_(entries, 0, true))
}

fn split_hash_merge_(entries: &[OwnedEntry], offset: usize, is_root: bool) -> [u8; 32] {
    if entries.len() > 64 {
        split_hash_merge::<RayonJoin>(entries, offset, is_root)
    } else {
        split_hash_merge::<SerialJoin>(entries, offset, is_root)
    }
}

fn split_hash_merge<J: Join>(entries: &[OwnedEntry], offset: usize, is_root: bool) -> [u8; 32] {
    assert!(!entries.is_empty());

    let n = entries.len();
    if n == 1 {
        let mut buffer = Vec::new();
        let entry = BorrowedEntry::from(&entries[0]);
        write_entry_transcript(&mut buffer, entry, offset as u16, is_root);
        return IV::DIR.hash_all_at_once(&buffer);
    }

    let split = previous_pow_of_two(n);
    let (left_cv, right_cv) = J::join(
        || split_hash_merge_(&entries[..split], offset, false),
        || split_hash_merge_(&entries[split..], offset + split, false),
    );

    IV::DIR.merge(&left_cv, &right_cv, is_root)
}
