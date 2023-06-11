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
    // Proof.
    // proof: Option<Vec<Blake3Hash>>,
    // TRee builder.
    // tree_builder: Option<HashTreeBuilder>,

    // Needed.
    buf: BytesMut,
    stack: Vec<Chunk>,
    store: MemoryBlockStore,
    // root: Option<Blake3Hash>,

    // We might be able to just use the length of the stack.
    block_counter: u32,

    mode: Mode,
}

enum Mode {
    // Both need to return root.
    Verify {
        proof: Vec<Blake3Hash>,
        root: Blake3Hash,
    },
    Trust {
        tree_builder: HashTreeBuilder,
    },
}

impl IncrementalPut {
    pub fn new(root: Option<Blake3Hash>, store: MemoryBlockStore) -> Self {
        Self {
            store,
            mode: Mode::Trust {
                tree_builder: HashTreeBuilder::new(),
            },
            stack: Vec::new(),
            block_counter: 0,
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
            let chunk = self.buf.split_to(256 * 1024);
            let mut block = BlockHasher::new();
            block.set_block(self.block_counter as usize);
            block.update(chunk.as_ref());

            match &mut self.mode {
                Mode::Verify { proof, root } => {
                    let mut verifier =
                        IncrementalVerifier::new(root.clone(), self.block_counter as usize);
                    let proof_buf = ProofBuf::new(proof, self.block_counter as usize);
                    verifier
                        .feed_proof(proof_buf.as_slice())
                        .map_err(|_| PutWriteError::InvalidContent)?;
                    verifier.verify(block.clone()).unwrap();
                },
                Mode::Trust { tree_builder } => tree_builder.update(chunk.as_ref()),
            }

            let hash = block.finalize(true); // Is this arg always true?
            // let chunk = match self.proof.as_ref() {
            //     Some(proof) => {
            //         let root = self.root.clone().expect("Root hash to have been provided");
            //         let mut verifier = IncrementalVerifier::new(root, self.block_counter as
            // usize);         let proof_buf = ProofBuf::new(proof, self.block_counter
            // as usize);         verifier
            //             .feed_proof(proof_buf.as_slice())
            //             .map_err(|_| PutWriteError::InvalidContent)?;
            //         verifier.verify(block.clone()).unwrap();
            //         let hash = block.finalize(true); // Is this arg always true?
            //         Chunk {
            //             hash,
            //             content: ContentChunk {
            //                 compression,
            //                 content: content.to_vec(),
            //             },
            //         }
            //     },
            //     None => {
            //         let hash = block.finalize(true); // Is this arg always true?
            //         let tree_builder = self
            //             .tree_builder
            //             .as_mut()
            //             .expect("There to be a tree builder");
            //         tree_builder.update(content.as_ref());
            //         Chunk {
            //             hash,
            //             content: ContentChunk {
            //                 compression,
            //                 content: content.to_vec(),
            //             },
            //         }
            //     },
            // };
            self.stack.push(Chunk {
                hash,
                content: ContentChunk {
                    compression,
                    content: chunk.to_vec(),
                },
            });
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

        match self.mode {
            Mode::Verify { root, .. } => Ok(root),
            Mode::Trust { tree_builder } => {
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
