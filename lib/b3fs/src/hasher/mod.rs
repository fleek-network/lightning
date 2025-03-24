//! Implementation of high performance blake3 hashers that keep track of the intermediary trees.
//!
//! This provides hashing ability both for content and directory.

pub(crate) mod b3;
pub mod collector;
pub(crate) mod join;

pub mod byte_hasher;
pub mod dir_hasher;
pub mod iv;

/// Any object that can intercept the intermediary hash tree and collect it.
#[allow(clippy::len_without_is_empty)]
pub trait HashTreeCollector {
    /// Push a new hash into the hash tree.
    fn push(&mut self, hash: [u8; 32]);

    /// Return the number of items we have inserted to this hash tree so far.
    fn len(&self) -> usize;

    /// Reserve space for `additional` new elements to be inserted.
    fn reserve(&mut self, additional: usize) {}
}

#[derive(Default)]
struct SkipHashTreeCollector(usize);

impl HashTreeCollector for Vec<[u8; 32]> {
    #[inline]
    fn push(&mut self, hash: [u8; 32]) {
        Vec::push(self, hash)
    }

    #[inline]
    fn reserve(&mut self, additional: usize) {
        Vec::reserve_exact(self, additional)
    }

    #[inline]
    fn len(&self) -> usize {
        Vec::len(self)
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

    #[inline]
    fn len(&self) -> usize {
        Vec::len(self) / 32
    }
}

impl HashTreeCollector for SkipHashTreeCollector {
    #[inline]
    fn push(&mut self, hash: [u8; 32]) {
        self.0 += 1;
    }

    #[inline]
    fn len(&self) -> usize {
        self.0
    }
}

// auto-impl the trait for any mut references to the hash tree.
impl<T: HashTreeCollector> HashTreeCollector for &mut T {
    fn push(&mut self, hash: [u8; 32]) {
        T::push(*self, hash)
    }

    fn len(&self) -> usize {
        T::len(*self)
    }

    fn reserve(&mut self, additional: usize) {
        T::reserve(*self, additional)
    }
}
