use std::marker::PhantomData;

use fastcrypto::hash::HashFunction;

use crate::verify::Verifier;

pub struct Sha256(PhantomData<()>);
pub struct Sha512(PhantomData<()>);
pub struct Blake3(PhantomData<()>);
pub struct Integrity<T>(T);

impl Integrity<Blake3> {
    pub fn verifier(self) -> Verifier<fastcrypto::hash::Blake3, 32> {
        Verifier::new(
            IncrementalBuilder::new(fastcrypto::hash::Blake3::new()),
            Vec::new(),
        )
    }
}

impl Integrity<Sha256> {
    pub fn verifier(self) -> Verifier<fastcrypto::hash::Sha256, 32> {
        Verifier::new(
            IncrementalBuilder::new(fastcrypto::hash::Sha256::new()),
            Vec::new(),
        )
    }
}

impl Integrity<Sha512> {
    pub fn verifier(self) -> Verifier<fastcrypto::hash::Sha512, 64> {
        Verifier::new(
            IncrementalBuilder::new(fastcrypto::hash::Sha512::new()),
            Vec::new(),
        )
    }
}

#[non_exhaustive]
pub enum IntegrityMetadata {
    Sha256(Integrity<Sha256>),
    Sha512(Integrity<Sha512>),
    Blake3(Integrity<Blake3>),
}

pub struct IncrementalBuilder<H: HashFunction<DIGEST_LEN>, const DIGEST_LEN: usize> {
    hash: H,
}

impl<H, const DIGEST_LEN: usize> IncrementalBuilder<H, DIGEST_LEN>
where
    H: HashFunction<DIGEST_LEN>,
{
    pub fn new(hash: H) -> Self {
        Self { hash }
    }

    pub fn update(&mut self) {}
}
