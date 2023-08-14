use std::{collections::HashMap, sync::Arc, marker::PhantomData};

use async_trait::async_trait;
use lightning_interfaces::{
    types::{CompressionAlgoSet, CompressionAlgorithm},
    Blake3Hash, Blake3Tree, BlockStoreInterface, ConfigConsumer, ContentChunk, infu_collection::Collection,
};
use parking_lot::RwLock;

use crate::{config::Config, put::IncrementalPut, store::Store, Block, BlockContent, Key};

#[derive(Clone, Default)]
pub struct MemoryBlockStore<C: Collection> {
    inner: Arc<RwLock<HashMap<Key, Block>>>,
    collection: PhantomData<C>
}

impl<C> ConfigConsumer for MemoryBlockStore<C> where C: Collection<BlockStoreInterface = Self> {
    const KEY: &'static str = "blockstore";
    type Config = Config;
}

#[async_trait]
impl<C> BlockStoreInterface for MemoryBlockStore<C> where C: Collection<BlockStoreInterface = Self> {
    type Collection = C;

    type SharedPointer<T: ?Sized + Send + Sync> = Arc<T>;
    type Put = IncrementalPut<Self>;

    fn init(_: Self::Config) -> anyhow::Result<Self> {
        Ok(Self {
            inner: Default::default(),
            collection: PhantomData
        })
    }

    async fn get_tree(&self, cid: &Blake3Hash) -> Option<Self::SharedPointer<Blake3Tree>> {
        match bincode::deserialize::<BlockContent>(
            self.fetch(&Key::tree_key(*cid)).await?.as_slice(),
        )
        .expect("Stored content to be serialized properly")
        {
            BlockContent::Tree(tree) => Some(Arc::new(Blake3Tree(tree))),
            _ => None,
        }
    }

    async fn get(
        &self,
        block_counter: u32,
        block_hash: &Blake3Hash,
        _compression: CompressionAlgoSet,
    ) -> Option<Self::SharedPointer<ContentChunk>> {
        match bincode::deserialize::<BlockContent>(
            self.fetch(&Key::chunk_key(*block_hash, block_counter))
                .await?
                .as_slice(),
        )
        .expect("Stored content to be serialized properly")
        {
            BlockContent::Chunk(content) => Some(Arc::new(ContentChunk {
                compression: CompressionAlgorithm::Uncompressed,
                content,
            })),
            _ => None,
        }
    }

    fn put(&self, root: Option<Blake3Hash>) -> Self::Put {
        match root {
            Some(root) => IncrementalPut::verifier(self.clone(), root),
            None => IncrementalPut::trust(self.clone()),
        }
    }
}

#[async_trait]
impl<C> Store for MemoryBlockStore<C> where C: Collection {
    async fn fetch(&self, key: &Key) -> Option<Block> {
        self.inner.read().get(key).cloned()
    }

    async fn insert(&mut self, key: Key, block: Block) {
        self.inner.write().insert(key, block);
    }
}
