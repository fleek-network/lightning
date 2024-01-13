use std::str::FromStr;

use base64::Engine;
use fastcrypto::hash::{Digest, HashFunction};

use crate::verify::Verifier;

const SHA256_ALGO: &str = "sha256";
const SHA512_ALGO: &str = "sha512";
const BLAKE3_ALGO: &str = "blake3";

#[non_exhaustive]
pub enum IntegrityMetadata {
    Sha256(Integrity<Sha256>),
    Sha512(Integrity<Sha512>),
    Blake3(Integrity<Blake3>),
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

pub struct Integrity<T> {
    digest: T,
}

pub struct Sha256(Digest<32>);
pub struct Sha512(Digest<64>);
pub struct Blake3(Digest<32>);

impl Integrity<Blake3> {
    pub fn verifier(self) -> Verifier<fastcrypto::hash::Blake3, 32> {
        Verifier::new(IncrementalBuilder::default(), self.digest.0)
    }
}

impl Integrity<Sha256> {
    pub fn verifier(self) -> Verifier<fastcrypto::hash::Sha256, 32> {
        Verifier::new(IncrementalBuilder::default(), self.digest.0)
    }
}

impl Integrity<Sha512> {
    pub fn verifier(self) -> Verifier<fastcrypto::hash::Sha512, 64> {
        Verifier::new(IncrementalBuilder::default(), self.digest.0)
    }
}

#[derive(Default)]
pub struct IncrementalBuilder<H: HashFunction<DIGEST_LEN>, const DIGEST_LEN: usize> {
    hash: H,
}

impl<H, const DIGEST_LEN: usize> IncrementalBuilder<H, DIGEST_LEN>
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

impl IncrementalBuilder<fastcrypto::hash::Sha256, 32> {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn finalize(self) -> Sha256 {
        Sha256(self.hash.finalize())
    }
}

impl IncrementalBuilder<fastcrypto::hash::Sha512, 64> {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn finalize(self) -> Sha512 {
        Sha512(self.hash.finalize())
    }
}

impl IncrementalBuilder<fastcrypto::hash::Blake3, 32> {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn finalize(self) -> Blake3 {
        Blake3(self.hash.finalize())
    }
}
