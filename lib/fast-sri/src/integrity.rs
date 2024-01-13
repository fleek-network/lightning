use std::marker::PhantomData;

use fastcrypto::hash::{Digest, HashFunction};

use crate::verify::Verifier;

#[non_exhaustive]
pub enum IntegrityMetadata {
    Sha256(Integrity<Sha256>),
    Sha512(Integrity<Sha512>),
    Blake3(Integrity<Blake3>),
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
