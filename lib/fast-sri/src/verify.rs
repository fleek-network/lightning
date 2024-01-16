use fastcrypto::hash::{Digest, HashFunction};

use crate::integrity::Builder;
use crate::IntegrityMetadata;

pub struct Verifier<H: HashFunction<DIGEST_LEN>, const DIGEST_LEN: usize> {
    builder: Builder<H, DIGEST_LEN>,
    digest: Digest<DIGEST_LEN>,
}

impl<H: HashFunction<DIGEST_LEN>, const DIGEST_LEN: usize> Verifier<H, DIGEST_LEN> {
    pub(crate) fn new(builder: Builder<H, DIGEST_LEN>, digest: Digest<DIGEST_LEN>) -> Self {
        Self { builder, digest }
    }

    pub fn update<T: AsRef<[u8]>>(&mut self, data: T) {
        self.builder.update(data);
    }

    pub fn verify(self) -> bool {
        self.digest == self.builder.inner_finalize()
    }
}

pub struct BufferedVerifier {
    buff: Vec<u8>,
    integrity_metadata: IntegrityMetadata,
}

impl BufferedVerifier {
    pub(crate) fn new(integrity_metadata: IntegrityMetadata) -> Self {
        Self {
            integrity_metadata,
            buff: Vec::new(),
        }
    }

    pub(crate) fn new_with_data(integrity_metadata: IntegrityMetadata, data: Vec<u8>) -> Self {
        Self {
            integrity_metadata,
            buff: data,
        }
    }

    pub fn update<T: AsRef<[u8]>>(&mut self, data: T) {
        self.buff.extend_from_slice(data.as_ref());
    }

    pub fn verify(self) -> (bool, Vec<u8>) {
        let is_valid = match self.integrity_metadata {
            IntegrityMetadata::Sha256(integrity) => {
                let mut verifier = integrity.verifier();
                verifier.update(self.buff.as_slice());
                verifier.verify()
            },
            IntegrityMetadata::Sha512(integrity) => {
                let mut verifier = integrity.verifier();
                verifier.update(self.buff.as_slice());
                verifier.verify()
            },
            IntegrityMetadata::Blake3(integrity) => {
                let mut verifier = integrity.verifier();
                verifier.update(self.buff.as_slice());
                verifier.verify()
            },
        };
        (is_valid, self.buff)
    }
}

#[cfg(test)]
mod tests {
    use crate::IntegrityMetadata;

    #[test]
    fn test_verify_sha256() {
        // Given: an integrity metadata.
        let integrity_metadata: IntegrityMetadata =
            "sha256-MV9b23bQeMQ7isAGTkoBZGErH853yGk0W/yUx1iU7dM="
                .parse()
                .unwrap();
        let IntegrityMetadata::Sha256(integrity) = integrity_metadata else {
            panic!("invalid hashing function");
        };

        // When: we feed to a verifier data corresponding to the digest in the integrity metadata.
        let mut verifier = integrity.verifier();
        verifier.update("Hello,");
        verifier.update(" world!");

        // Then: verifies that digest is valid for the data.
        assert!(verifier.verify());

        // Given: an integrity metadata.
        let integrity_metadata: IntegrityMetadata =
            "sha256-MV9b23bQeMQ7isAGTkoBZGErH853yGk0W/yUx1iU7dM="
                .parse()
                .unwrap();
        let IntegrityMetadata::Sha256(integrity) = integrity_metadata else {
            panic!("invalid hashing function");
        };

        // When: we feed to a verifier data that doesn't correspond to the digest in the integrity
        // metadata.
        let mut verifier = integrity.verifier();
        verifier.update("foo");
        verifier.update("bar");

        // Then: verifies that digest is invalid for the data.
        assert!(!verifier.verify());
    }

    #[test]
    fn test_verify_sha512() {
        // Given: an integrity metadata.
        let integrity_metadata: IntegrityMetadata =  "sha512-wVJ82JPBJHc9gRkRlwyP5uhX1t9dySJr2KFgYUwM2WOk3eorlLt9NgIe+dhl1c6ilKgt1JoLsmn1H256V/eUIQ==".parse().unwrap();
        let IntegrityMetadata::Sha512(integrity) = integrity_metadata else {
            panic!("invalid hashing function");
        };

        // When: we feed to a verifier data corresponding to the digest in the integrity metadata.
        let mut verifier = integrity.verifier();
        verifier.update("Hello,");
        verifier.update(" world!");

        // Then: verifies that digest is valid for the data.
        assert!(verifier.verify());

        // Given: an integrity metadata.
        let integrity_metadata: IntegrityMetadata =  "sha512-wVJ82JPBJHc9gRkRlwyP5uhX1t9dySJr2KFgYUwM2WOk3eorlLt9NgIe+dhl1c6ilKgt1JoLsmn1H256V/eUIQ==".parse().unwrap();
        let IntegrityMetadata::Sha512(integrity) = integrity_metadata else {
            panic!("invalid hashing function");
        };

        // When: we feed to a verifier data that doesn't correspond to the digest in the integrity
        // metadata.
        let mut verifier = integrity.verifier();
        verifier.update("foo");
        verifier.update("bar");

        // Then: verifies that digest is invalid for the data.
        assert!(!verifier.verify());
    }

    #[test]
    fn test_verify_blake3() {
        // Given: an integrity metadata.
        let integrity_metadata: IntegrityMetadata =
            "blake3-7eXAsQ8uxJecabUvYeQv9bQTUZzgm+DxTQmNz+X2+Y0="
                .parse()
                .unwrap();
        let IntegrityMetadata::Blake3(integrity) = integrity_metadata else {
            panic!("invalid hashing function");
        };

        // When: we feed to a verifier data corresponding to the digest in the integrity metadata.
        let mut verifier = integrity.verifier();
        verifier.update("Hello,");
        verifier.update(" world!");

        // Then: verifies that digest is valid for the data.
        assert!(verifier.verify());

        // Given: an integrity metadata.
        let integrity_metadata: IntegrityMetadata =
            "blake3-7eXAsQ8uxJecabUvYeQv9bQTUZzgm+DxTQmNz+X2+Y0="
                .parse()
                .unwrap();
        let IntegrityMetadata::Blake3(integrity) = integrity_metadata else {
            panic!("invalid hashing function");
        };

        // When: we feed to a verifier data that doesn't correspond to the digest in the integrity
        // metadata.
        let mut verifier = integrity.verifier();
        verifier.update("foo");
        verifier.update("bar!");

        // Then: verifies that digest is invalid for the data.
        assert!(!verifier.verify());
    }
}
