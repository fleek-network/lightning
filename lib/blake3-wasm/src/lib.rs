use arrayref::array_ref;
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn hash(buffer: Vec<u8>) -> Vec<u8> {
    let hash: [u8; 32] = blake3_tree::blake3::hash(&buffer).into();
    Vec::from(hash.as_slice())
}

#[wasm_bindgen]
pub struct IncrementalVerifier {
    inner: blake3_tree::IncrementalVerifier,
    block_counter: usize,
}

#[wasm_bindgen]
impl IncrementalVerifier {
    #[wasm_bindgen(constructor)]
    pub fn new(root_hash: Vec<u8>, offset: usize) -> Self {
        assert_eq!(root_hash.len(), 32);

        let root_hash = *array_ref![root_hash, 0, 32];

        Self {
            inner: blake3_tree::IncrementalVerifier::new(root_hash, offset),
            block_counter: offset,
        }
    }

    /// Feed some proof bytes to the verifier. Panics if the proof is invalid.
    pub fn feed_proof(&mut self, proof: Vec<u8>) {
        self.feed_proof_inner(proof.as_ref());
    }

    /// Feed a full block of content to the verifier. Panics if the content is invalid.
    pub fn feed_content(&mut self, content: Vec<u8>) {
        self.feed_content_inner(content.as_ref());
    }

    /// Feed proof and content in a single call. The content must be full. The proof bytes
    /// indicates the number of bytes for the proof.
    pub fn feed_proof_and_content(&mut self, proof_bytes: usize, buffer: Vec<u8>) {
        let proof = &buffer[0..proof_bytes];
        let content = &buffer[proof_bytes..];
        self.feed_proof_inner(proof);
        self.feed_content_inner(content);
    }
}

impl IncrementalVerifier {
    #[inline(never)]
    fn feed_proof_inner(&mut self, proof: &[u8]) {
        self.inner.feed_proof(proof).expect("invalid proof");
    }

    #[inline(never)]
    fn feed_content_inner(&mut self, content: &[u8]) {
        let mut block = blake3_tree::blake3::tree::BlockHasher::new();
        block.set_block(self.block_counter);
        block.update(content);
        self.inner.verify(block).expect("invalid content");
        self.block_counter += 1;
    }
}
