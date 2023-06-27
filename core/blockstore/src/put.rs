use async_trait::async_trait;
use blake3_tree::{
    blake3::tree::{BlockHasher, HashTreeBuilder},
    IncrementalVerifier,
};
use bytes::{BufMut, Bytes, BytesMut};
use draco_interfaces::{
    Blake3Hash, CompressionAlgorithm, ContentChunk, IncrementalPutInterface, PutFeedProofError,
    PutFinalizeError, PutWriteError,
};

use crate::{store::Store, BlockContent, Key, BLAKE3_CHUNK_SIZE};

struct Chunk {
    hash: Blake3Hash,
    content: ContentChunk,
}

pub struct IncrementalPut<S> {
    buf: BytesMut,
    prev_block: Option<(BlockHasher, ContentChunk)>,
    stack: Vec<Chunk>,
    store: S,
    mode: Mode,
}

enum Mode {
    Verify {
        proof: Option<Bytes>,
        root: Blake3Hash,
        verifier: IncrementalVerifier,
    },
    Trust {
        tree_builder: Box<HashTreeBuilder>,
    },
}

impl<S> IncrementalPut<S>
where
    S: Store,
{
    pub fn verifier(store: S, root: Blake3Hash) -> Self {
        Self::internal_new(
            store,
            Mode::Verify {
                proof: None,
                root,
                verifier: IncrementalVerifier::new(root, 0),
            },
        )
    }

    pub fn trust(store: S) -> Self {
        Self::internal_new(
            store,
            Mode::Trust {
                tree_builder: Box::new(HashTreeBuilder::new()),
            },
        )
    }

    fn internal_new(store: S, mode: Mode) -> Self {
        Self {
            store,
            mode,
            prev_block: None,
            stack: Vec::new(),
            buf: BytesMut::new(),
        }
    }
}

#[async_trait]
impl<S> IncrementalPutInterface for IncrementalPut<S>
where
    S: Store + Send,
{
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
            let block_count = self.stack.len() + self.prev_block.as_ref().map(|_| 1).unwrap_or(0);
            let chunk = self.buf.split_to(BLAKE3_CHUNK_SIZE);
            let mut block = BlockHasher::new();
            block.set_block(block_count);
            block.update(chunk.as_ref());

            match &mut self.mode {
                Mode::Verify {
                    proof, verifier, ..
                } => {
                    verifier
                        .feed_proof(
                            proof
                                .as_ref()
                                // TODO: We need a better error here.
                                .ok_or(PutWriteError::InvalidContent)?,
                        )
                        .map_err(|_| PutWriteError::InvalidContent)?;
                    verifier
                        .verify(block.clone())
                        .map_err(|_| PutWriteError::InvalidContent)?;
                },
                Mode::Trust { tree_builder } => tree_builder.update(chunk.as_ref()),
            }

            if let Some((prev_block, prev_content_chunk)) = self.prev_block.take() {
                let hash = prev_block.finalize(false);
                self.stack.push(Chunk {
                    hash,
                    content: prev_content_chunk,
                });
            }

            let content_chunk = ContentChunk {
                compression,
                content: chunk.to_vec(),
            };
            self.prev_block = Some((block, content_chunk));
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
        let (prev_block, prev_content_chunk) = self
            .prev_block
            .ok_or_else(|| PutFinalizeError::PartialContent)?;
        let is_root = self.stack.len() == 0;
        let hash = prev_block.finalize(is_root);
        self.stack.push(Chunk {
            hash,
            content: prev_content_chunk,
        });

        // TODO: put methods use a non-async lock so these calls could
        // block the thread. Maybe let's use the worker pattern.
        for (count, chunk) in self.stack.into_iter().enumerate() {
            let block = bincode::serialize(&BlockContent::Chunk(chunk.content.content))
                .map_err(|_| PutFinalizeError::PartialContent)?;
            self.store
                .store_put(Key::chunk_key(chunk.hash, count as u32), block);
        }

        match self.mode {
            Mode::Verify { root, .. } => Ok(root),
            Mode::Trust { tree_builder } => {
                let hash_tree = tree_builder.finalize();
                let block = bincode::serialize(&BlockContent::Tree(hash_tree.tree))
                    .map_err(|_| PutFinalizeError::PartialContent)?;
                self.store.store_put(
                    Key::tree_key(Blake3Hash::from(hash_tree.hash)),
                    // TODO: We need a more descriptive error for serialization-related errors.
                    block,
                );
                Ok(Blake3Hash::from(hash_tree.hash))
            },
        }
    }
}
