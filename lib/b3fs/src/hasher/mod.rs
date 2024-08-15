//! Implementation of high performance blake3 hashers that keep track of the intermediary trees.
//!
//! This provides hashing ability both for content and directory.

pub(crate) mod b3;
pub(crate) mod join;

pub mod byte_hasher;
pub mod dir_hasher;
pub mod iv;

/// Any object that can intercept the intermediary hash tree and collect it.
pub trait HashTreeCollector {
    fn push(&mut self, hash: [u8; 32]);

    fn reserve(&mut self, additional: usize) {}
}

impl HashTreeCollector for Vec<[u8; 32]> {
    #[inline]
    fn push(&mut self, hash: [u8; 32]) {
        Vec::push(self, hash)
    }

    #[inline]
    fn reserve(&mut self, additional: usize) {
        Vec::reserve_exact(self, additional)
    }
}

impl HashTreeCollector for Vec<u8> {
    #[inline]
    fn push(&mut self, hash: [u8; 32]) {
        Vec::extend_from_slice(self, &hash)
    }

    #[inline]
    fn reserve(&mut self, additional: usize) {
        Vec::reserve_exact(self, additional * 32)
    }
}
