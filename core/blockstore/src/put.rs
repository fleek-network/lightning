use std::sync::Arc;

use async_trait::async_trait;
use blake3_tree::{
    blake3::tree::{BlockHasher, HashTreeBuilder},
    IncrementalVerifier, ProofBuf,
};
use bytes::{BufMut, BytesMut};
use draco_interfaces::{
    Blake3Hash, BlockStoreInterface, CompressionAlgorithm, ContentChunk, IncrementalPutInterface,
    PutFeedProofError, PutFinalizeError, PutWriteError,
};

use crate::{memory::MemoryBlockStore, Block, Key};

struct Chunk {
    hash: Blake3Hash,
    content: ContentChunk,
}

pub struct IncrementalPut<B> {
    store: MemoryBlockStore<B>,
    proof: Option<ProofBuf>,
    root: Option<Blake3Hash>,
    tree_builder: Option<HashTreeBuilder>,
    stack: Vec<Chunk>,
    block_counter: u32,
    buf: BytesMut,
}

impl<B> IncrementalPut<B>
where
    B: BlockStoreInterface,
{
    pub fn new(root: Option<Blake3Hash>, store: MemoryBlockStore<B>) -> Self {
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
impl<B> IncrementalPutInterface for IncrementalPut<B>
where
    B: BlockStoreInterface + Send,
{
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
                self.proof = Some(ProofBuf::new(&(tree.clone().0), 0));
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
                block.update(content.as_ref());
                block
            };

            let chunk = match self.proof.as_ref() {
                Some(proof) => {
                    let root = self.root.clone().expect("Root hash to have been provided");
                    // Verify.
                    let mut verifier = IncrementalVerifier::new(root, self.block_counter as usize);
                    verifier
                        .feed_proof(proof.as_slice())
                        .map_err(|_| PutWriteError::InvalidContent)?;
                    verifier
                        .verify(block.clone())
                        .map_err(|_| PutWriteError::InvalidContent)?;
                    let is_root = verifier.is_root();
                    let hash = block.finalize(is_root); // Is this always true?
                    Chunk {
                        hash,
                        content: ContentChunk {
                            compression,
                            content: content.to_vec(),
                        },
                    }
                },
                None => {
                    let hash = block.finalize(true); // Is this always true?
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
            Ok(Blake3Hash::from(tree_builder.finalize().hash))
        } else {
            self.root.ok_or_else(|| PutFinalizeError::PartialContent)
        }
    }
}
