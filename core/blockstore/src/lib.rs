use std::{sync::Arc, collections::HashMap};
use draco_interfaces::{Blake3Hash, Blake3Tree, BlockStoreInterface, CompressionAlgorithm, CompressionAlgoSet, ConfigConsumer, ContentChunk, IncrementalPutInterface, PutFeedProofError, PutFinalizeError, PutWriteError};
use serde::{Deserialize, Serialize};
use parking_lot::RwLock;
use async_trait::async_trait;


#[derive(Serialize, Deserialize, Default)]
pub struct Config;

#[derive(Clone)]
struct Blockstore {
    inner: Arc<RwLock<HashMap<Blake3Hash, Block>>>,
}

impl ConfigConsumer for Blockstore {
    const KEY: &'static str = "blockstore";
    type Config = Config;
}

struct IncrementalPut;

#[async_trait]
impl IncrementalPutInterface for IncrementalPut {
    fn feed_proof(&mut self, _proof: &[u8]) -> Result<(), PutFeedProofError> {
        todo!()
    }

    fn write(&mut self, _content: &[u8], _compression: CompressionAlgorithm) -> Result<(), PutWriteError> {
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

#[async_trait]
impl BlockStoreInterface for Blockstore {
    type SharedPointer<T: ?Sized + Send + Sync> = Arc<T>;
    type Put = IncrementalPut;

    async fn init(_: Self::Config) -> anyhow::Result<Self> {
        Ok(Self {
            inner: Default::default()
        })
    }

    async fn get_tree(&self, cid: &Blake3Hash) -> Option<Self::SharedPointer<Blake3Tree>> {
        match self.inner.read().get(cid)? {
            Block::Tree(tree) => Some(tree.clone()),
            Block::Chunk(_) => None,
        }
    }

    async fn get(&self, _block_counter: u32, block_hash: &Blake3Hash, _compression: CompressionAlgoSet) -> Option<Self::SharedPointer<ContentChunk>> {
        match self.inner.read().get(block_hash)? {
            Block::Chunk(chunk) => Some(chunk.clone()),
            Block::Tree(_) => None,
        }
    }

    fn put(&self, _cid: Option<Blake3Hash>) -> Self::Put {
        todo!()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn put_basic() {}

    #[test]
    fn get_basic() {}
}
