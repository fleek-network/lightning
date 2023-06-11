use std::sync::Arc;

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

use crate::{memory::MemoryBlockStore, Block, Key};

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
        match self.store.inner.read().get(&Key(root, None)) {
            Some(Block::Tree(tree)) => {
                self.mode = Mode::Verify {
                    proof: tree.clone().0.clone(),
                    root,
                };
                Ok(())
            },
            _ => Err(PutFeedProofError::InvalidProof),
        }
    }

    fn write(
        &mut self,
        content: &[u8],
        compression: CompressionAlgorithm,
    ) -> Result<(), PutWriteError> {
        self.buf.put(content);

        while self.buf.len() >= 256 * 1024 {
            let block_count = self.stack.len();
            let chunk = self.buf.split_to(256 * 1024);
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
                    verifier.verify(block.clone()).unwrap();
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
            self.store.inner.write().insert(
                Key(chunk.hash, Some(count as u32)),
                Block::Chunk(Arc::new(chunk.content)),
            );
        }

        match self.mode {
            Mode::Verify { root, .. } => Ok(root),
            Mode::BuildHashTree { tree_builder } => {
                let hash_tree = tree_builder.finalize();
                self.store.inner.write().insert(
                    Key(Blake3Hash::from(hash_tree.hash), None),
                    Block::Tree(Arc::new(Blake3Tree(hash_tree.tree))),
                );
                Ok(Blake3Hash::from(hash_tree.hash))
            },
        }
    }
}
