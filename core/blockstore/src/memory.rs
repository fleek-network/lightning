use std::{collections::HashMap, marker::PhantomData, sync::Arc};

use async_trait::async_trait;
use draco_interfaces::{
    Blake3Hash, Blake3Tree, BlockStoreInterface, CompressionAlgoSet, ConfigConsumer, ContentChunk,
};
use parking_lot::RwLock;

use crate::{config::Config, put::IncrementalPut, Block, Key};

#[derive(Clone)]
pub struct MemoryBlockStore<B> {
    pub(crate) inner: Arc<RwLock<HashMap<Key, Block>>>,
    data: PhantomData<B>,
}

impl<B> ConfigConsumer for MemoryBlockStore<B> {
    const KEY: &'static str = "blockstore";
    type Config = Config;
}

#[async_trait]
impl<B: BlockStoreInterface> BlockStoreInterface for MemoryBlockStore<B> {
    type SharedPointer<T: ?Sized + Send + Sync> = Arc<T>;
    type Put = IncrementalPut<B>;

    async fn init(_: Self::Config) -> anyhow::Result<Self> {
        Ok(Self {
            inner: Default::default(),
            data: PhantomData,
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
