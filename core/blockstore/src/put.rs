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
    store: MemoryBlockStore,
    proof: Option<Vec<Blake3Hash>>,
    root: Option<Blake3Hash>,
    tree_builder: Option<HashTreeBuilder>,
    stack: Vec<Chunk>,
    block_counter: u32,
    buf: BytesMut,
}

impl IncrementalPut {
    pub fn new(root: Option<Blake3Hash>, store: MemoryBlockStore) -> Self {
        Self {
            store,
            root,
            proof: None,
            tree_builder: Some(HashTreeBuilder::new()),
            stack: Vec::new(),
            block_counter: 0,
            buf: BytesMut::new(),
        }
    }
}

#[async_trait]
impl IncrementalPutInterface for IncrementalPut {
    fn feed_proof(&mut self, proof: &[u8]) -> Result<(), PutFeedProofError> {
        if (self.buf.len() > 0 || self.block_counter > 0) && self.proof.is_none() {
            return Err(PutFeedProofError::UnexpectedCall);
        }
        let root = proof
            .try_into()
            .map_err(|_| PutFeedProofError::InvalidProof)?;
        match self.store.inner.read().get(&Key(root, None)) {
            None | Some(Block::Chunk(_)) => Err(PutFeedProofError::InvalidProof),
            Some(Block::Tree(tree)) => {
                self.proof = Some(tree.clone().0.clone());
                self.root = Some(root);
                Ok(())
            },
        }
    }

    fn write(
        &mut self,
        content: &[u8],
        compression: CompressionAlgorithm,
    ) -> Result<(), PutWriteError> {
        self.buf.put(content);

        while self.buf.len() >= 256 * 1024 {
            let content = self.buf.split_to(256 * 1024);
            let block = {
                let mut block = BlockHasher::new();
                block.set_block(self.block_counter as usize);
                block.update(content.as_ref());
                block
            };

            let chunk = match self.proof.as_ref() {
                Some(proof) => {
                    self.tree_builder = None;
                    let root = self.root.clone().expect("Root hash to have been provided");
                    let mut verifier = IncrementalVerifier::new(root, self.block_counter as usize);
                    let proof_buf = ProofBuf::new(proof, self.block_counter as usize);
                    verifier
                        .feed_proof(proof_buf.as_slice())
                        .map_err(|_| PutWriteError::InvalidContent)?;
                    verifier.verify(block.clone()).unwrap();
                    let hash = block.finalize(true); // Is this arg always true?
                    Chunk {
                        hash,
                        content: ContentChunk {
                            compression,
                            content: content.to_vec(),
                        },
                    }
                },
                None => {
                    let hash = block.finalize(true); // Is this arg always true?
                    let tree_builder = self
                        .tree_builder
                        .as_mut()
                        .expect("There to be a tree builder");
                    tree_builder.update(content.as_ref());
                    Chunk {
                        hash,
                        content: ContentChunk {
                            compression,
                            content: content.to_vec(),
                        },
                    }
                },
            };
            self.stack.push(chunk);
            self.block_counter += 1
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
        if let Some(tree_builder) = self.tree_builder {
            let hash_tree = tree_builder.finalize();
            self.store.inner.write().insert(
                Key(Blake3Hash::from(hash_tree.hash), None),
                Block::Tree(Arc::new(Blake3Tree(hash_tree.tree))),
            );
            Ok(Blake3Hash::from(hash_tree.hash))
        } else {
            self.root.ok_or_else(|| PutFinalizeError::PartialContent)
        }
    }
}
