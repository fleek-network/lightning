use std::sync::Arc;

use async_trait::async_trait;
use draco_interfaces::{
    blockstore::BlockStoreInterface, common::WithStartAndShutdown, config::ConfigConsumer,
    Blake3Hash, Blake3Tree, CompressionAlgoSet, ContentChunk, IncrementalPutInterface,
};

use super::config::Config;

#[derive(Clone)]
pub struct BlockStore {}

#[async_trait]
impl WithStartAndShutdown for BlockStore {
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

pub struct MyPut;

#[async_trait]
impl IncrementalPutInterface for MyPut {
    fn feed_proof(&mut self, _proof: &[u8]) -> Result<(), draco_interfaces::PutFeedProofError> {
        todo!()
    }

    fn write(
        &mut self,
        _content: &[u8],
        _compression: draco_interfaces::CompressionAlgorithm,
    ) -> Result<(), draco_interfaces::PutWriteError> {
        todo!()
    }

    fn is_finished(&self) -> bool {
        todo!()
    }

    async fn finalize(
        self,
    ) -> Result<draco_interfaces::Blake3Hash, draco_interfaces::PutFinalizeError> {
        todo!()
    }
}

#[async_trait]
impl BlockStoreInterface for BlockStore {
    /// The block store has the ability to use a smart pointer to avoid duplicating
    /// the same content multiple times in memory, this can be used for when multiple
    /// services want access to the same buffer of data.
    type SharedPointer<T: ?Sized + Send + Sync> = Arc<T>;

    /// The incremental putter which can be used to write a file to block store.
    type Put = MyPut;

    /// Create a new block store from the given configuration values.
    async fn init(_config: Self::Config) -> anyhow::Result<Self> {
        todo!()
    }

    /// Returns the Blake3 tree associated with the given CID. Returns [`None`] if the content
    /// is not present in our block store.
    async fn get_tree(&self, _cid: &Blake3Hash) -> Option<Self::SharedPointer<Blake3Tree>> {
        todo!()
    }

    /// Returns the content associated with the given hash and block number, the compression
    /// set determines which compression modes we care about.
    ///
    /// The strongest compression should be preferred. The cache logic should take note of
    /// the number of requests for a `CID` + the supported compression so that it can optimize
    /// storage by storing the compressed version.
    ///
    /// If the content is requested with an empty compression set, the decompressed content is
    /// returned.
    async fn get(
        &self,
        _block_counter: u32,
        _block_hash: &Blake3Hash,
        _compression: CompressionAlgoSet,
    ) -> Option<Self::SharedPointer<ContentChunk>> {
        todo!()
    }

    /// Create a putter that can be used to write a content into the block store.
    fn put(&self, _cid: Option<Blake3Hash>) -> Self::Put {
        todo!()
    }
}

impl ConfigConsumer for BlockStore {
    const KEY: &'static str = "blockstore";

    type Config = Config;
}
