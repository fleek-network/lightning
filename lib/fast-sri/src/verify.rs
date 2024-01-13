use fastcrypto::hash::{Digest, HashFunction};

use crate::integrity::IncrementalBuilder;

pub struct Verifier<H: HashFunction<DIGEST_LEN>, const DIGEST_LEN: usize> {
    builder: IncrementalBuilder<H, DIGEST_LEN>,
    digest: Digest<DIGEST_LEN>,
}

impl<H: HashFunction<DIGEST_LEN>, const DIGEST_LEN: usize> Verifier<H, DIGEST_LEN> {
    pub(crate) fn new(
        builder: IncrementalBuilder<H, DIGEST_LEN>,
        digest: Digest<DIGEST_LEN>,
    ) -> Self {
        Self { builder, digest }
    }

    pub fn update<T: AsRef<[u8]>>(&mut self, data: T) {
        self.builder.update(data);
    }

    pub fn verify(self) -> bool {
        self.digest == self.builder.inner_finalize()
    }
}
