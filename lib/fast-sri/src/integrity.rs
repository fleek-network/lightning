use std::str::FromStr;

use base64::Engine;
use fastcrypto::hash::{Digest, HashFunction};

use crate::verify::Verifier;
use crate::BufferedVerifier;

const SHA256_ALGO: &str = "sha256";
const SHA512_ALGO: &str = "sha512";
const BLAKE3_ALGO: &str = "blake3";

#[non_exhaustive]
pub enum IntegrityMetadata {
    Sha256(Integrity<Sha256>),
    Sha512(Integrity<Sha512>),
    Blake3(Integrity<Blake3>),
}

impl IntegrityMetadata {
    pub fn into_verifier(self) -> BufferedVerifier {
        BufferedVerifier::new(self)
    }
}

impl FromStr for IntegrityMetadata {
    type Err = std::io::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let (algo, digest) = s.split_once('-').ok_or(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "invalid integrity metadata syntax",
        ))?;
        match algo {
            SHA256_ALGO => {
                let digest: [u8; 32] = base64::prelude::BASE64_STANDARD
                    .decode(digest)
                    .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?
                    .try_into()
                    .map_err(|_| {
                        std::io::Error::new(std::io::ErrorKind::InvalidData, "invalid digest")
                    })?;
                Ok(IntegrityMetadata::Sha256(Integrity {
                    digest: Sha256(Digest::new(digest)),
                }))
            },
            SHA512_ALGO => {
                let digest: [u8; 64] = base64::prelude::BASE64_STANDARD
                    .decode(digest)
                    .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?
                    .try_into()
                    .map_err(|_| {
                        std::io::Error::new(std::io::ErrorKind::InvalidData, "invalid digest")
                    })?;
                Ok(IntegrityMetadata::Sha512(Integrity {
                    digest: Sha512(Digest::new(digest)),
                }))
            },
            BLAKE3_ALGO => {
                let digest: [u8; 32] = base64::prelude::BASE64_STANDARD
                    .decode(digest)
                    .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?
                    .try_into()
                    .map_err(|_| {
                        std::io::Error::new(std::io::ErrorKind::InvalidData, "invalid digest")
                    })?;
                Ok(IntegrityMetadata::Blake3(Integrity {
                    digest: Blake3(Digest::new(digest)),
                }))
            },
            algo => Err(std::io::Error::new(
                std::io::ErrorKind::Unsupported,
                format!("unsupported algo: {algo}"),
            )),
        }
    }
}

impl std::fmt::Display for IntegrityMetadata {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            IntegrityMetadata::Sha256(integrity) => integrity.fmt(f),
            IntegrityMetadata::Sha512(integrity) => integrity.fmt(f),
            IntegrityMetadata::Blake3(integrity) => integrity.fmt(f),
        }
    }
}

pub struct Integrity<T> {
    digest: T,
}

impl Integrity<Sha256> {
    pub fn builder() -> Builder<fastcrypto::hash::Sha256, 32> {
        Builder::default()
    }

    pub fn verifier(self) -> Verifier<fastcrypto::hash::Sha256, 32> {
        Verifier::new(Builder::default(), self.digest.0)
    }
}

impl std::fmt::Display for Integrity<Sha256> {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        let digest = base64::prelude::BASE64_STANDARD.encode(self.digest.0.digest);
        write!(f, "sha256-{}", digest)
    }
}

impl Integrity<Sha512> {
    pub fn builder() -> Builder<fastcrypto::hash::Sha512, 64> {
        Builder::default()
    }

    pub fn verifier(self) -> Verifier<fastcrypto::hash::Sha512, 64> {
        Verifier::new(Builder::default(), self.digest.0)
    }
}

impl std::fmt::Display for Integrity<Sha512> {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        let digest = base64::prelude::BASE64_STANDARD.encode(self.digest.0.digest);
        write!(f, "sha512-{}", digest)
    }
}

// Todo: since this type is specific to blake3,
// we can extend this impl to expose features unique to blake3
// such as incremental verification.
impl Integrity<Blake3> {
    pub fn builder() -> Builder<fastcrypto::hash::Blake3, 32> {
        Builder::default()
    }

    pub fn verifier(self) -> Verifier<fastcrypto::hash::Blake3, 32> {
        Verifier::new(Builder::default(), self.digest.0)
    }
}

impl std::fmt::Display for Integrity<Blake3> {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        let digest = base64::prelude::BASE64_STANDARD.encode(self.digest.0.digest);
        write!(f, "blake3-{}", digest)
    }
}

#[derive(PartialEq, Eq)]
pub struct Sha256(Digest<32>);
#[derive(PartialEq, Eq)]
pub struct Sha512(Digest<64>);
#[derive(PartialEq, Eq)]
pub struct Blake3(Digest<32>);

#[derive(Default)]
pub struct Builder<H: HashFunction<DIGEST_LEN>, const DIGEST_LEN: usize> {
    hash: H,
}

impl<H, const DIGEST_LEN: usize> Builder<H, DIGEST_LEN>
where
    H: HashFunction<DIGEST_LEN>,
{
    pub fn update<T: AsRef<[u8]>>(&mut self, data: T) {
        self.hash.update(data);
    }

    pub(crate) fn inner_finalize(self) -> Digest<DIGEST_LEN> {
        self.hash.finalize()
    }
}

impl Builder<fastcrypto::hash::Sha256, 32> {
    pub fn finalize(self) -> Integrity<Sha256> {
        Integrity {
            digest: Sha256(self.hash.finalize()),
        }
    }
}

impl Builder<fastcrypto::hash::Sha512, 64> {
    pub fn finalize(self) -> Integrity<Sha512> {
        Integrity {
            digest: Sha512(self.hash.finalize()),
        }
    }
}

impl Builder<fastcrypto::hash::Blake3, 32> {
    pub fn finalize(self) -> Integrity<Blake3> {
        Integrity {
            digest: Blake3(self.hash.finalize()),
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::{Blake3, Integrity, IntegrityMetadata, Sha256, Sha512};

    #[test]
    fn test_builder() {
        let mut builder = Integrity::<Sha256>::builder();
        builder.update("Hello, world!");
        assert_eq!(
            builder.finalize().to_string().as_str(),
            "sha256-MV9b23bQeMQ7isAGTkoBZGErH853yGk0W/yUx1iU7dM="
        );

        let mut builder = Integrity::<Sha512>::builder();
        builder.update("Hello, world!");
        assert_eq!(
            builder.finalize().to_string().as_str(),
            "sha512-wVJ82JPBJHc9gRkRlwyP5uhX1t9dySJr2KFgYUwM2WOk3eorlLt9NgIe+dhl1c6ilKgt1JoLsmn1H256V/eUIQ=="
        );

        let mut builder = Integrity::<Blake3>::builder();
        builder.update("Hello, world!");
        assert_eq!(
            builder.finalize().to_string().as_str(),
            "blake3-7eXAsQ8uxJecabUvYeQv9bQTUZzgm+DxTQmNz+X2+Y0="
        );

        let mut builder = Integrity::<Sha256>::builder();
        builder.update("Hello,");
        builder.update(" world!");
        assert_eq!(
            builder.finalize().to_string().as_str(),
            "sha256-MV9b23bQeMQ7isAGTkoBZGErH853yGk0W/yUx1iU7dM="
        );

        let mut builder = Integrity::<Sha512>::builder();
        builder.update("Hello,");
        builder.update(" world!");
        assert_eq!(
            builder.finalize().to_string().as_str(),
            "sha512-wVJ82JPBJHc9gRkRlwyP5uhX1t9dySJr2KFgYUwM2WOk3eorlLt9NgIe+dhl1c6ilKgt1JoLsmn1H256V/eUIQ=="
        );

        let mut builder = Integrity::<Blake3>::builder();
        builder.update("Hello,");
        builder.update(" world!");
        assert_eq!(
            builder.finalize().to_string().as_str(),
            "blake3-7eXAsQ8uxJecabUvYeQv9bQTUZzgm+DxTQmNz+X2+Y0="
        );
    }

    #[test]
    fn test_integrity_metadata_parsing() {
        let integrity: IntegrityMetadata = "sha256-MV9b23bQeMQ7isAGTkoBZGErH853yGk0W/yUx1iU7dM="
            .parse()
            .unwrap();
        assert!(matches!(integrity, IntegrityMetadata::Sha256(_)));

        let integrity: IntegrityMetadata = "sha512-wVJ82JPBJHc9gRkRlwyP5uhX1t9dySJr2KFgYUwM2WOk3eorlLt9NgIe+dhl1c6ilKgt1JoLsmn1H256V/eUIQ==".parse().unwrap();
        assert!(matches!(integrity, IntegrityMetadata::Sha512(_)));

        let integrity: IntegrityMetadata = "blake3-7eXAsQ8uxJecabUvYeQv9bQTUZzgm+DxTQmNz+X2+Y0="
            .parse()
            .unwrap();
        assert!(matches!(integrity, IntegrityMetadata::Blake3(_)));

        // Invalid digest.
        assert!("sha256-foo".parse::<IntegrityMetadata>().is_err());

        // Unsupported hash function.
        assert!(
            "sha384-VbxVaw0v4Pzlgrpf4Huq//A1ZTY4x6wNVJTCpkwL6hzFczHHwSpFzbyn9MNKCJ7r"
                .parse::<IntegrityMetadata>()
                .is_err()
        );
    }
}
