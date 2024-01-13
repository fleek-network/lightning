use fastcrypto::hash::HashFunction;

use crate::integrity::IncrementalBuilder;

pub struct Verifier<H: HashFunction<DIGEST_LEN>, const DIGEST_LEN: usize> {
    builder: IncrementalBuilder<H, DIGEST_LEN>,
    digest: Vec<u8>,
}

impl<H: HashFunction<DIGEST_LEN>, const DIGEST_LEN: usize> Verifier<H, DIGEST_LEN> {
    pub fn new(builder: IncrementalBuilder<H, DIGEST_LEN>, digest: Vec<u8>) -> Self {
        Self { builder, digest }
    }

    pub fn update<T: AsRef<[u8]>>(&mut self, data: T) {
        todo!()
    }

    pub fn verify(self) -> bool {
        todo!()
    }
}
