#[derive(Hash, Debug, PartialEq, Eq, Clone, Copy)]
#[repr(u8)]
pub enum CompressionAlgorithm {
    Uncompressed = 0,
    Snappy = 1,
    Gzip = 2,
    Brotli = 4,
    Lz4 = 8,
    Lzma = 16,
}

/// A set of [`CompressionAlgorithm`] values. The [`CompressionAlgorithm::Uncompressed`]
/// is a special case
#[derive(Hash, Default, Debug, PartialEq, Eq, PartialOrd, Ord, Clone, Copy)]
pub struct CompressionAlgoSet(u8);

impl CompressionAlgoSet {
    /// Create a new empty set.
    pub fn new() -> Self {
        Self(0)
    }

    /// Insert the provided algorithm to this set.
    pub fn insert(&mut self, algo: CompressionAlgorithm) {
        self.0 |= algo as u8;
    }

    /// Remove the given algorithm from the set.
    pub fn remove(&mut self, algo: CompressionAlgorithm) {
        self.0 &= !(algo as u8);
    }

    /// Returns true if the given algorithm is present in this set.
    pub fn contains(&self, algo: CompressionAlgorithm) -> bool {
        (self.0 & (algo as u8)) != 0 || algo == CompressionAlgorithm::Uncompressed
    }

    /// Returns the intersection of this set with another set.
    pub fn intersect(&self, other: &Self) -> Self {
        Self(self.0 & other.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compression_set() {
        let mut set = CompressionAlgoSet::new();
        // {}
        assert!(set.contains(CompressionAlgorithm::Uncompressed));
        assert!(!set.contains(CompressionAlgorithm::Snappy));
        assert!(!set.contains(CompressionAlgorithm::Gzip));
        assert!(!set.contains(CompressionAlgorithm::Brotli));
        assert!(!set.contains(CompressionAlgorithm::Lz4));
        assert!(!set.contains(CompressionAlgorithm::Lzma));
        // {Lz4}
        set.insert(CompressionAlgorithm::Lz4);
        assert!(set.contains(CompressionAlgorithm::Uncompressed));
        assert!(!set.contains(CompressionAlgorithm::Snappy));
        assert!(!set.contains(CompressionAlgorithm::Gzip));
        assert!(!set.contains(CompressionAlgorithm::Brotli));
        assert!(set.contains(CompressionAlgorithm::Lz4));
        assert!(!set.contains(CompressionAlgorithm::Lzma));
        // {Lz4, Brotli}
        set.insert(CompressionAlgorithm::Brotli);
        assert!(set.contains(CompressionAlgorithm::Uncompressed));
        assert!(!set.contains(CompressionAlgorithm::Snappy));
        assert!(!set.contains(CompressionAlgorithm::Gzip));
        assert!(set.contains(CompressionAlgorithm::Brotli));
        assert!(set.contains(CompressionAlgorithm::Lz4));
        assert!(!set.contains(CompressionAlgorithm::Lzma));
        // {Lz4}
        set.remove(CompressionAlgorithm::Brotli);
        assert!(set.contains(CompressionAlgorithm::Uncompressed));
        assert!(!set.contains(CompressionAlgorithm::Snappy));
        assert!(!set.contains(CompressionAlgorithm::Gzip));
        assert!(!set.contains(CompressionAlgorithm::Brotli));
        assert!(set.contains(CompressionAlgorithm::Lz4));
        assert!(!set.contains(CompressionAlgorithm::Lzma));
        // {}
        set.remove(CompressionAlgorithm::Lz4);
        assert!(set.contains(CompressionAlgorithm::Uncompressed));
        assert!(!set.contains(CompressionAlgorithm::Snappy));
        assert!(!set.contains(CompressionAlgorithm::Gzip));
        assert!(!set.contains(CompressionAlgorithm::Brotli));
        assert!(!set.contains(CompressionAlgorithm::Lz4));
        assert!(!set.contains(CompressionAlgorithm::Lzma));
    }
}
