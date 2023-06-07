mod config;

use std::{collections::HashMap, sync::Arc};

use async_trait::async_trait;
use draco_interfaces::{
    Blake3Hash, Blake3Tree, BlockStoreInterface, CompressionAlgoSet, CompressionAlgorithm,
    ConfigConsumer, ContentChunk, IncrementalPutInterface, PutFeedProofError, PutFinalizeError,
    PutWriteError,
};
use parking_lot::RwLock;
use crate::config::Config;

#[derive(Hash, Eq, PartialEq)]
pub struct Key<'a>(&'a Blake3Hash, Option<u32>);

#[derive(Clone)]
struct MemoryBlockStore {
    inner: Arc<RwLock<HashMap<Key<'static>, Block>>>,
}

impl ConfigConsumer for MemoryBlockStore {
    const KEY: &'static str = "blockstore";
    type Config = Config;
}

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

#[async_trait]
impl BlockStoreInterface for MemoryBlockStore {
    type SharedPointer<T: ?Sized + Send + Sync> = Arc<T>;
    type Put = IncrementalPut;

    async fn init(_: Self::Config) -> anyhow::Result<Self> {
        Ok(Self {
            inner: Default::default(),
        })
    }

    async fn get_tree(&self, cid: &Blake3Hash) -> Option<Self::SharedPointer<Blake3Tree>> {
        match self.inner.read().get(&Key(cid, None))? {
            Block::Tree(tree) => Some(tree.clone()),
            _ => None,
        }
    }

    async fn get(
        &self,
        block_counter: u32,
        block_hash: &Blake3Hash,
        _compression: CompressionAlgoSet,
    ) -> Option<Self::SharedPointer<ContentChunk>> {
        match self
            .inner
            .read()
            .get(&Key(block_hash, Some(block_counter)))?
        {
            Block::Chunk(chunk) => Some(chunk.clone()),
            _ => None,
        }
    }

    fn put(&self, _cid: Option<Blake3Hash>) -> Self::Put {
        todo!()
    }
}

#[cfg(test)]
mod tests {
}
