use bytes::Bytes;
use fleek_blake3::tree::HashTreeBuilder;

use crate::store::store::Store;
use crate::verifier::IncrementalVerifier;

pub struct FilePutter {
    store: Store,
    expected_hash: Option<[u8; 32]>,
    mode: Mode,
}

enum Mode {
    WithVerifier(IncrementalVerifier),
    WithHasher(HashTreeBuilder),
}

impl FilePutter {
    pub fn new(store: Store, expected_hash: Option<[u8; 32]>) {
        todo!()
    }

    pub fn feed_proof(&self, proof: &[u8]) {}

    pub fn write(&self, content: Bytes) {}
}
