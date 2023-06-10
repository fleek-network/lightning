use std::sync::Arc;

use async_trait::async_trait;
use blake3_tree::{
    blake3::tree::{BlockHasher, HashTreeBuilder},
    IncrementalVerifier, ProofBuf,
};
use bytes::{BufMut, Bytes, BytesMut};
use draco_interfaces::{
    Blake3Hash, BlockStoreInterface, CompressionAlgorithm, ContentChunk, IncrementalPutInterface,
    PutFeedProofError, PutFinalizeError, PutWriteError,
};

use crate::{memory::MemoryBlockStore, Block, Key};

pub struct IncrementalPut<B> {
    store: MemoryBlockStore<B>,
    proof: Option<(ProofBuf, Blake3Hash)>,
    block_counter: u32,
    buf: BytesMut,
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
        match self.store.inner.read().get(&Key(
            proof
                .try_into()
                .map_err(|_| PutFeedProofError::InvalidProof)?,
            None,
        )) {
            None | Some(Block::Chunk(_)) => Err(PutFeedProofError::InvalidProof),
            Some(Block::Tree(tree)) => {
                self.proof = Some((
                    ProofBuf::new(&(tree.clone().0), 0),
                    *tree
                        .0
                        .last()
                        .ok_or_else(|| PutFeedProofError::InvalidProof)?,
                ));
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
            let chunk = ContentChunk {
                compression,
                content: content.to_vec(),
            };
            let block = {
                let mut block = BlockHasher::new();
                block.update(content.as_ref());
                block
            };
            if let Some((proof, root)) = self.proof.as_ref() {
                // Verify.
                let mut verifier = IncrementalVerifier::new(*root, self.block_counter as usize);
                let is_root = verifier.is_root();
                verifier
                    .verify(block.clone())
                    .map_err(|_| PutWriteError::InvalidContent)?;

                let cv = {
                    let mut block = BlockHasher::new();
                    block.update(content.as_ref());
                    block.finalize(is_root)
                };
                // let content = content.to_vec();
                self.store.inner.write().insert(
                    Key(
                        cv,
                        Some(self.block_counter),
                    ),
                    Block::Chunk(Arc::new(chunk)),
                );
            } else {
                // Build tree.
            }
        }

        Ok(())
    }

    fn is_finished(&self) -> bool {
        todo!()
    }

    async fn finalize(self) -> Result<Blake3Hash, PutFinalizeError> {
        todo!()
    }
}
