use std::{collections::HashMap, sync::Arc};

use async_trait::async_trait;
use draco_interfaces::{
    Blake3Hash, Blake3Tree, BlockStoreInterface, CompressionAlgoSet, CompressionAlgorithm,
    ConfigConsumer, ContentChunk,
};
use parking_lot::RwLock;
use thiserror::Error;

use crate::{config::Config, put::IncrementalPut, Block, BlockContent, Key};

#[derive(Error, Debug)]
#[error("Serialization failed")]
pub struct SerializationError;

#[derive(Clone, Default)]
pub struct MemoryBlockStore {
    inner: Arc<RwLock<HashMap<Key, Block>>>,
}

impl MemoryBlockStore {
    pub fn basic_get(&self, key: &Key) -> Option<ContentChunk> {
        match bincode::deserialize::<BlockContent>(self.inner.read().get(key)?)
            .expect("Stored content to be serialized properly")
        {
            BlockContent::Chunk(content) => Some(ContentChunk {
                compression: CompressionAlgorithm::Uncompressed,
                content,
            }),
            _ => None,
        }
    }

    pub fn basic_get_tree(&self, key: &Key) -> Option<Blake3Tree> {
        match bincode::deserialize::<BlockContent>(self.inner.read().get(key)?)
            .expect("Stored content to be serialized properly")
        {
            BlockContent::Tree(tree) => Some(Blake3Tree(tree)),
            _ => None,
        }
    }

    pub fn basic_put_tree(
        &self,
        key: Key,
        tree: Blake3Tree,
    ) -> Result<Option<Block>, SerializationError> {
        let block =
            bincode::serialize(&BlockContent::Tree(tree.0)).map_err(|_| SerializationError)?;
        Ok(self.inner.write().insert(key, block))
    }

    pub fn basic_put(
        &self,
        key: Key,
        chunk: ContentChunk,
    ) -> Result<Option<Block>, SerializationError> {
        let block = bincode::serialize(&BlockContent::Chunk(chunk.content))
            .map_err(|_| SerializationError)?;
        Ok(self.inner.write().insert(key, block))
    }
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
        self.basic_get_tree(&Key::tree_key(*cid)).map(Arc::new)
    }

    async fn get(
        &self,
        block_counter: u32,
        block_hash: &Blake3Hash,
        _compression: CompressionAlgoSet,
    ) -> Option<Self::SharedPointer<ContentChunk>> {
        self.basic_get(&Key::chunk_key(*block_hash, block_counter))
            .map(Arc::new)
    }

    fn put(&self, _: Option<Blake3Hash>) -> Self::Put {
        IncrementalPut::new(self.clone())
    }
}
