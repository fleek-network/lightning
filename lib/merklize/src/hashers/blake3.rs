use crate::SimpleHasher;

/// A hasher that uses the Blake3 algorithm.
#[derive(Clone)]
pub struct Blake3Hasher(blake3::Hasher);

impl SimpleHasher for Blake3Hasher {
    const ICS23_HASH_OP: ics23::HashOp = ics23::HashOp::Blake3;

    fn new() -> Self {
        Self(blake3::Hasher::new())
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
    fn test_blake3_hasher() {
        let mut hasher = Blake3Hasher::new();
        assert_eq!(Blake3Hasher::ICS23_HASH_OP, ics23::HashOp::Blake3);

        hasher.update(b"hello");
        hasher.update(b"world");
        let hash = hasher.finalize();
        assert_eq!(
            hash,
            [
                123, 178, 5, 36, 77, 128, 131, 86, 49, 142, 198, 93, 10, 229, 79, 50, 238, 58, 123,
                171, 93, 250, 244, 49, 176, 30, 86, 126, 3, 186, 171, 79
            ]
        );
        assert_eq!(hash.len(), 32);
    }
}
