use async_trait::async_trait;
use blake3_tree::{
    blake3::tree::{BlockHasher, HashTreeBuilder},
    IncrementalVerifier,
};
use bytes::{BufMut, Bytes, BytesMut};
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
        proof: Option<Bytes>,
        root: Blake3Hash,
    },
    Trust {
        tree_builder: Box<HashTreeBuilder>,
    },
}

impl IncrementalPut {
    pub fn verifier(store: MemoryBlockStore, root: Blake3Hash) -> Self {
        Self::internal_new(store, Mode::Verify { proof: None, root })
    }

    pub fn trust(store: MemoryBlockStore) -> Self {
        Self::internal_new(
            store,
            Mode::Trust {
                tree_builder: Box::new(HashTreeBuilder::new()),
            },
        )
    }

    fn internal_new(store: MemoryBlockStore, mode: Mode) -> Self {
        Self {
            store,
            mode,
            stack: Vec::new(),
            buf: BytesMut::new(),
        }
    }
}

#[async_trait]
impl IncrementalPutInterface for IncrementalPut {
    fn feed_proof(&mut self, proof: &[u8]) -> Result<(), PutFeedProofError> {
        match &mut self.mode {
            Mode::Verify {
                proof: inner_proof, ..
            } => match inner_proof {
                Some(_) => return Err(PutFeedProofError::UnexpectedCall),
                None => {
                    inner_proof.replace(Bytes::copy_from_slice(proof));
                },
            },
            Mode::Trust { .. } => {
                return Err(PutFeedProofError::UnexpectedCall);
            },
        }
        Ok(())
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
                    let mut verifier = IncrementalVerifier::new(*root, block_count);
                    verifier
                        .feed_proof(
                            proof
                                .as_ref()
                                // TODO: We need a better error here.
                                .ok_or_else(|| PutWriteError::InvalidContent)?,
                        )
                        .map_err(|_| PutWriteError::InvalidContent)?;
                    verifier
                        .verify(block.clone())
                        .map_err(|_| PutWriteError::InvalidContent)?;
                },
                Mode::Trust { tree_builder } => tree_builder.update(chunk.as_ref()),
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

        // Remove this proof so it's ready for next block.
        match &mut self.mode {
            Mode::Verify { proof, .. } if self.buf.is_empty() => {
                proof.take();
            },
            _ => {},
        }

        Ok(())
    }

    fn is_finished(&self) -> bool {
        // Since self is consumed when calling `finalize`, this would always return false.
        false
    }

    async fn finalize(mut self) -> Result<Blake3Hash, PutFinalizeError> {
        // TODO: put methods use a non-async lock so these calls could
        // block the thread. Maybe let's use the worker pattern.
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
            Mode::Trust { tree_builder } => {
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
