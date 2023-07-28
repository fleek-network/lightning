use async_trait::async_trait;
use lightning_blockstore::memory::MemoryBlockStore;
use lightning_interfaces::{
    Blake3Hash, Blake3Tree, BlockStoreInterface, CompressionAlgoSet, ConfigConsumer, ContentChunk,
    FileSystemInterface, WithStartAndShutdown,
};

use crate::empty_interfaces::{MockConfig, MockIndexer};

#[derive(Clone)]
pub struct MockFileSystem {
    store: MemoryBlockStore,
}

#[async_trait]
impl WithStartAndShutdown for MockFileSystem {
    /// Returns true if this system is running or not.
    fn is_running(&self) -> bool {
        true
    }

    /// Start the system, should not do anything if the system is already
    /// started.
    async fn start(&self) {}

    /// Send the shutdown signal to the system.
    async fn shutdown(&self) {}
}

#[async_trait]
impl FileSystemInterface for MockFileSystem {
    /// The block store used for this file system.
    type BlockStore = MemoryBlockStore;

    /// The indexer used for this file system.
    type Indexer = MockIndexer;

    fn new(store: &Self::BlockStore, _indexer: &Self::Indexer) -> Self {
        Self {
            store: store.clone(),
        }
    }

    /// Returns true if the given `cid` is already cached on the node.
    fn is_cached(&self, _cid: &Blake3Hash) {}

    /// Returns the tree of the provided cid.
    async fn get_tree(
        &self,
        cid: &Blake3Hash,
    ) -> Option<<Self::BlockStore as BlockStoreInterface>::SharedPointer<Blake3Tree>> {
        self.store.get_tree(cid).await
    }

    /// Returns the requested chunk of data.
    async fn get(
        &self,
        block_counter: u32,
        block_hash: &Blake3Hash,
        compression: CompressionAlgoSet,
    ) -> Option<<Self::BlockStore as BlockStoreInterface>::SharedPointer<ContentChunk>> {
        self.store.get(block_counter, block_hash, compression).await
    }

    async fn request_download(&self, _cid: &Blake3Hash) -> bool {
        todo!()
    }
}

impl ConfigConsumer for MockFileSystem {
    const KEY: &'static str = "fs";

    type Config = MockConfig;
}
