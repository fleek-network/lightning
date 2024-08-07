//! An implementation of CHD algorithm borrowed mostly from the `phf` crate and adjusted for our
//! use case.

use crate::directory::BorrowedEntry;

struct PhfState {
    pub key: u64,
    pub disp: Vec<(u16, u16)>,
    pub map: Vec<u32>,
}

pub fn generate<'a, E: Into<BorrowedEntry<'a>>>(entries: &'a [E]) {}

/// The hash function used for hashing the strings.
fn hash_bytes(key: u64, bytes: &[u8]) -> u64 {
    0
}
