use std::sync::Arc;

use async_trait::async_trait;
use lightning_interfaces::{
    types::{CompressionAlgoSet, CompressionAlgorithm},
    Blake3Hash, Blake3Tree, BlockStoreInterface, ConfigConsumer, ContentChunk,
};
use serde::{Deserialize, Serialize};
use tempdir::TempDir;
use tokio::{
    fs::{self, File},
    io::AsyncWriteExt,
};

use crate::{put::IncrementalPut, store::Store, Block, BlockContent, Key};

const TMP_DIR_PREFIX: &str = "tmp-store";

#[derive(Serialize, Deserialize, Default)]
pub struct FsStoreConfig {
    store_dir_path: String,
}

#[derive(Clone)]
pub struct FsStore {
    store_dir_path: String,
    tmp_dir: Arc<TempDir>,
}

impl ConfigConsumer for FsStore {
    const KEY: &'static str = "fsstore";
    type Config = FsStoreConfig;
}

#[async_trait]
impl BlockStoreInterface for FsStore {
    type SharedPointer<T: ?Sized + Send + Sync> = Arc<T>;
    type Put = IncrementalPut<Self>;

    async fn init(config: Self::Config) -> anyhow::Result<Self> {
        Ok(Self {
            store_dir_path: config.store_dir_path,
            tmp_dir: TempDir::new(TMP_DIR_PREFIX).map(Arc::new)?,
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

// TODO: Add logging.
#[async_trait]
impl Store for FsStore {
    async fn fetch(&self, key: &Key) -> Option<Block> {
        let path = format!("{}/{:?}", self.store_dir_path, key.0);
        fs::read(path).await.ok()
    }

    // TODO: This should perhaps return an error.
    async fn insert(&mut self, key: Key, block: Block) {
        let filename = format!("{:?}", key.0);
        let path = self.tmp_dir.path().join(filename);
        if let Ok(mut tmp_file) = File::create(&path).await {
            if tmp_file.write_all(block.as_ref()).await.is_err() {
                return;
            }
            // TODO: Is this needed before calling rename?
            if tmp_file.sync_all().await.is_err() {
                return;
            }
            let store_path = format!("{}/{:?}", self.store_dir_path, key.0);
            if fs::rename(path, store_path).await.is_err() {
                return;
            }
        }
    }
}
