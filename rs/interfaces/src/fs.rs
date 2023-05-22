use async_trait::async_trait;

use crate::{
    blockstore::{Blake3Hash, Blake3Tree, BlockStoreInterface, ContentChunk},
    compression::CompressionAlgoSet,
    indexer::IndexerInterface,
};

#[async_trait]
pub trait FileSystemInterface: Clone {
    /// The block store used for this file system.
    type BlockStore: BlockStoreInterface;

    /// The indexer used for this file system.
    type Indexer: IndexerInterface;

    fn new(store: &Self::BlockStore, indexer: &Self::Indexer) -> Self;

    /// Returns true if the given `cid` is already cached on the node.
    fn is_cached(&self, cid: &Blake3Hash);

    /// Returns the tree of the provided cid.
    async fn get_tree(
        &self,
        cid: &Blake3Hash,
    ) -> Option<<Self::BlockStore as BlockStoreInterface>::SharedPointer<Blake3Tree>>;

    /// Returns the requested chunk of data.
    async fn get(
        &self,
        block_counter: u32,
        block_hash: &Blake3Hash,
        compression: CompressionAlgoSet,
    ) -> Option<<Self::BlockStore as BlockStoreInterface>::SharedPointer<ContentChunk>>;

    async fn request_download(&self, cid: &Blake3Hash) -> bool;
}
