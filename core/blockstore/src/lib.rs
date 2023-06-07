mod config;
mod memory;

use std::sync::Arc;
use async_trait::async_trait;
use draco_interfaces::{
    Blake3Hash, Blake3Tree, CompressionAlgorithm,
    ContentChunk, IncrementalPutInterface, PutFeedProofError, PutFinalizeError,
    PutWriteError,
};

#[derive(Hash, Eq, PartialEq)]
pub struct Key<'a>(&'a Blake3Hash, Option<u32>);

struct IncrementalPut;

#[async_trait]
impl IncrementalPutInterface for IncrementalPut {
    fn feed_proof(&mut self, _proof: &[u8]) -> Result<(), PutFeedProofError> {
        todo!()
    }

    fn write(
        &mut self,
        _content: &[u8],
        _compression: CompressionAlgorithm,
    ) -> Result<(), PutWriteError> {
        todo!()
    }

    fn is_finished(&self) -> bool {
        todo!()
    }

    async fn finalize(self) -> Result<Blake3Hash, PutFinalizeError> {
        todo!()
    }
}

pub enum Block {
    Tree(Arc<Blake3Tree>),
    Chunk(Arc<ContentChunk>),
}
