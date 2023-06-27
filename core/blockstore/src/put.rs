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
    content_buf: BytesMut,
    prev_block: Option<(BlockHasher, ContentChunk)>,
    chunks: Vec<Chunk>,
    store: S,
    mode: Mode,
    compression: Option<CompressionAlgorithm>,
    block_count: usize,
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
            chunks: Vec::new(),
            content_buf: BytesMut::new(),
            compression: None,
            block_count: 0,
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
        self.content_buf.put(content);

        while self.content_buf.len() >= BLAKE3_CHUNK_SIZE {
            let chunk = self.content_buf.split_to(BLAKE3_CHUNK_SIZE);
            let mut block = BlockHasher::new();
            block.set_block(self.block_count);
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
                self.chunks.push(Chunk {
                    hash,
                    content: prev_content_chunk,
                });
            }

            let content_chunk = ContentChunk {
                compression,
                content: chunk.to_vec(),
            };
            self.prev_block = Some((block, content_chunk));
            self.block_count += 1;
        }

        // Remove this proof so it's ready for next block.
        match &mut self.mode {
            Mode::Verify { proof, .. } if self.content_buf.is_empty() => {
                proof.take();
            },
            _ => {},
        }

        if self.compression.is_none() {
            self.compression = Some(compression);
        }

        Ok(())
    }

    fn is_finished(&self) -> bool {
        // Since self is consumed when calling `finalize`, this would always return false.
        false
    }

    async fn finalize(mut self) -> Result<Blake3Hash, PutFinalizeError> {
        match self.prev_block {
            None => {
                if self.content_buf.is_empty() {
                    return Err(PutFinalizeError::PartialContent);
                }
            },
            Some((prev_block, prev_content_chunk)) => {
                // If the buf is not empty, it means we have some data smaller than a
                // Blake3 chunk size that needs to be processed so this buffered block
                // is not the root.
                let is_root = self.block_count == 0 && self.content_buf.is_empty();
                let hash = prev_block.finalize(is_root);
                self.chunks.push(Chunk {
                    hash,
                    content: prev_content_chunk,
                });
            },
        }

        // Check if there is some data left that we haven't pushed in the stack.
        // This data is smaller than a Blake3 chunk size.
        if !self.content_buf.is_empty() {
            let compression = self
                .compression
                .expect("user to have specified a compression algorithm");
            let mut block = BlockHasher::new();
            block.set_block(self.chunks.len());
            block.update(self.content_buf.as_ref());
            let is_root = self.chunks.is_empty();
            let hash = block.finalize(is_root);
            self.chunks.push(Chunk {
                hash,
                content: ContentChunk {
                    compression,
                    content: self.content_buf.to_vec(),
                },
            });
        }

        // TODO: put methods use a non-async lock so these calls could
        // block the thread. Maybe let's use the worker pattern.
        for (count, chunk) in self.chunks.into_iter().enumerate() {
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
