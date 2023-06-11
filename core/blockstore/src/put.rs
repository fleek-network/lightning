use async_trait::async_trait;
use blake3_tree::{
    blake3::tree::{BlockHasher, HashTreeBuilder},
    IncrementalVerifier, ProofBuf,
};
use bytes::{BufMut, BytesMut};
use draco_interfaces::{
    Blake3Hash, Blake3Tree, CompressionAlgorithm, ContentChunk, IncrementalPutInterface,
    PutFeedProofError, PutFinalizeError, PutWriteError,
};

use crate::{memory::MemoryBlockStore, Key, BLAKE3_CHUNK_SIZE};

struct Chunk {
    hash: Blake3Hash,
    content: ContentChunk,
}

pub struct IncrementalPut {
    buf: BytesMut,
    stack: Vec<Chunk>,
    store: MemoryBlockStore,
    mode: Mode,
}

enum Mode {
    Verify {
        proof: Vec<Blake3Hash>,
        root: Blake3Hash,
    },
    BuildHashTree {
        tree_builder: HashTreeBuilder,
    },
}

impl IncrementalPut {
    pub fn new(store: MemoryBlockStore) -> Self {
        Self {
            store,
            mode: Mode::BuildHashTree {
                tree_builder: HashTreeBuilder::new(),
            },
            stack: Vec::new(),
            buf: BytesMut::new(),
        }
    }
}

#[async_trait]
impl IncrementalPutInterface for IncrementalPut {
    fn feed_proof(&mut self, proof: &[u8]) -> Result<(), PutFeedProofError> {
        if self.buf.len() > 0 || self.stack.len() > 0 {
            return Err(PutFeedProofError::UnexpectedCall);
        }
        let root = proof
            .try_into()
            .map_err(|_| PutFeedProofError::InvalidProof)?;
        match self.store.basic_get_tree(&Key::tree_key(root)) {
            Some(tree) => {
                self.mode = Mode::Verify {
                    proof: tree.0,
                    root,
                };
                Ok(())
            },
            None => Err(PutFeedProofError::InvalidProof),
        }
    }

    fn write(
        &mut self,
        content: &[u8],
        compression: CompressionAlgorithm,
    ) -> Result<(), PutWriteError> {
        self.buf.put(content);

        while self.buf.len() >= BLAKE3_CHUNK_SIZE {
            let block_count = self.stack.len();
            let chunk = self.buf.split_to(BLAKE3_CHUNK_SIZE);
            let mut block = BlockHasher::new();
            block.set_block(block_count);
            block.update(chunk.as_ref());

            match &mut self.mode {
                Mode::Verify { proof, root } => {
                    let mut verifier = IncrementalVerifier::new(root.clone(), block_count);
                    let proof_buf = ProofBuf::new(proof, block_count);
                    verifier
                        .feed_proof(proof_buf.as_slice())
                        .map_err(|_| PutWriteError::InvalidContent)?;
                    verifier
                        .verify(block.clone())
                        .map_err(|_| PutWriteError::InvalidContent)?;
                },
                Mode::BuildHashTree { tree_builder } => tree_builder.update(chunk.as_ref()),
            }

            let hash = block.finalize(true); // Is this arg always true?
            self.stack.push(Chunk {
                hash,
                content: ContentChunk {
                    compression,
                    content: chunk.to_vec(),
                },
            });
        }

        Ok(())
    }

    fn is_finished(&self) -> bool {
        // Since self is consumed when calling `finalize`, this would always return false.
        false
    }

    async fn finalize(mut self) -> Result<Blake3Hash, PutFinalizeError> {
        for (count, chunk) in self.stack.into_iter().enumerate() {
            self.store
                .basic_put(
                    Key::chunk_key(chunk.hash, count as u32),
                    // TODO: We need a more descriptive error for serialization-related errors.
                    chunk.content,
                )
                .map_err(|_| PutFinalizeError::InvalidCID)?;
        }

        match self.mode {
            Mode::Verify { root, .. } => Ok(root),
            Mode::BuildHashTree { tree_builder } => {
                let hash_tree = tree_builder.finalize();
                self.store
                    .basic_put_tree(
                        Key::tree_key(Blake3Hash::from(hash_tree.hash)),
                        // TODO: We need a more descriptive error for serialization-related errors.
                        Blake3Tree(hash_tree.tree),
                    )
                    .map_err(|_| PutFinalizeError::InvalidCID)?;
                Ok(Blake3Hash::from(hash_tree.hash))
            },
        }
    }
}
