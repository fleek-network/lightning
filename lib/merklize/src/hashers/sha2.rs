use sha2::Digest;

use crate::SimpleHasher;

/// A hasher that uses the Sha256 algorithm.
#[derive(Clone)]
pub struct Sha256Hasher(sha2::Sha256);

impl SimpleHasher for Sha256Hasher {
    const ICS23_HASH_OP: ics23::HashOp = ics23::HashOp::Sha256;

    fn new() -> Self {
        Self(sha2::Sha256::new())
    }

    fn update(&mut self, data: &[u8]) {
        self.0.update(data);
    }

    fn finalize(self) -> [u8; 32] {
        self.0.finalize().into()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sha256_hasher() {
        let mut hasher = Sha256Hasher::new();
        assert_eq!(Sha256Hasher::ICS23_HASH_OP, ics23::HashOp::Sha256);

        hasher.update(b"hello");
        hasher.update(b"world");
        let hash = hasher.finalize();
        assert_eq!(
            hash,
            [
                147, 106, 24, 92, 170, 162, 102, 187, 156, 190, 152, 30, 158, 5, 203, 120, 205,
                115, 43, 11, 50, 128, 235, 148, 68, 18, 187, 111, 143, 143, 7, 175
            ]
        );
        assert_eq!(hash.len(), 32);
    }
}
