use async_trait::async_trait;
use lightning_blockstore::memory::MemoryBlockStore;
use lightning_interfaces::{
    blockstore::BlockStoreInterface, common::WithStartAndShutdown, config::ConfigConsumer,
    fs::FileSystemInterface, Blake3Hash, Blake3Tree, CompressionAlgoSet, ContentChunk,
};

use super::config::Config;
use crate::Indexer;

#[derive(Clone)]
pub struct FileSystem {}

#[async_trait]
impl WithStartAndShutdown for FileSystem {
    /// Returns true if this system is running or not.
    fn is_running(&self) -> bool {
        todo!()
    }

    /// Start the system, should not do anything if the system is already
    /// started.
    async fn start(&self) {
        todo!()
    }

    /// Send the shutdown signal to the system.
    async fn shutdown(&self) {
        todo!()
    }
}

#[async_trait]
impl FileSystemInterface for FileSystem {
    /// The block store used for this file system.
    type BlockStore = MemoryBlockStore;

    /// The indexer used for this file system.
    type Indexer = Indexer;

    fn new(_store: &Self::BlockStore, _indexer: &Self::Indexer) -> Self {
        Self {}
    }

    /// Returns true if the given `cid` is already cached on the node.
    fn is_cached(&self, _cid: &Blake3Hash) {
        todo!()
    }

    /// Returns the tree of the provided cid.
    async fn get_tree(
        &self,
        _cid: &Blake3Hash,
    ) -> Option<<Self::BlockStore as BlockStoreInterface>::SharedPointer<Blake3Tree>> {
        todo!()
    }

    /// Returns the requested chunk of data.
    async fn get(
        &self,
        _block_counter: u32,
        _block_hash: &Blake3Hash,
        _compression: CompressionAlgoSet,
    ) -> Option<<Self::BlockStore as BlockStoreInterface>::SharedPointer<ContentChunk>> {
        todo!()
    }

    async fn request_download(&self, _cid: &Blake3Hash) -> bool {
        todo!()
    }
}

impl ConfigConsumer for FileSystem {
    const KEY: &'static str = "fs";

    type Config = Config;
}
