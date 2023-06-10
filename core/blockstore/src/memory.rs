use std::{collections::HashMap, sync::Arc};

use async_trait::async_trait;
use draco_interfaces::{
    Blake3Hash, Blake3Tree, BlockStoreInterface, CompressionAlgoSet, ConfigConsumer, ContentChunk,
};
use parking_lot::RwLock;

use crate::{config::Config, put::IncrementalPut, Block, Key};

#[derive(Clone, Default)]
pub struct MemoryBlockStore {
    pub(crate) inner: Arc<RwLock<HashMap<Key, Block>>>,
}

impl ConfigConsumer for MemoryBlockStore {
    const KEY: &'static str = "blockstore";
    type Config = Config;
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
        match self.inner.read().get(&Key(cid.clone(), None))? {
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
            .get(&Key(*block_hash, Some(block_counter)))?
        {
            Block::Chunk(chunk) => Some(chunk.clone()),
            _ => None,
        }
    }

    fn put(&self, cid: Option<Blake3Hash>) -> Self::Put {
        IncrementalPut::new(cid, self.clone())
    }
}
