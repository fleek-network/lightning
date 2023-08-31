use std::marker::PhantomData;
use std::path::PathBuf;
use std::sync::Arc;

use async_trait::async_trait;
use blake3_tree::blake3::Hash;
use lightning_interfaces::infu_collection::Collection;
use lightning_interfaces::types::{CompressionAlgoSet, CompressionAlgorithm};
use lightning_interfaces::{
    Blake3Hash,
    Blake3Tree,
    BlockStoreInterface,
    ConfigConsumer,
    ContentChunk,
};
use resolved_pathbuf::ResolvedPathBuf;
use serde::{Deserialize, Serialize};
use tempdir::TempDir;
use tokio::fs::{self, File};
use tokio::io::AsyncWriteExt;

use crate::put::IncrementalPut;
use crate::store::Store;
use crate::{Block, BlockContent, Key};

const TMP_DIR_PREFIX: &str = "tmp-store";

#[derive(Serialize, Deserialize)]
pub struct FsStoreConfig {
    pub store_dir_path: ResolvedPathBuf,
}

impl Default for FsStoreConfig {
    fn default() -> Self {
        Self {
            store_dir_path: "~/.lightning/data/blockstore"
                .try_into()
                .expect("Failed to resolve path"),
        }
    }
}

#[derive(Clone)]
pub struct FsStore<C: Collection> {
    store_dir_path: PathBuf,
    tmp_dir: Arc<TempDir>,
    collection: PhantomData<C>,
}

impl<C: Collection> ConfigConsumer for FsStore<C> {
    const KEY: &'static str = "fsstore";
    type Config = FsStoreConfig;
}

#[async_trait]
impl<C: Collection> BlockStoreInterface<C> for FsStore<C> {
    type SharedPointer<T: ?Sized + Send + Sync> = Arc<T>;
    type Put = IncrementalPut<Self>;

    fn init(config: Self::Config) -> anyhow::Result<Self> {
        std::fs::create_dir_all(config.store_dir_path.clone())?;
        Ok(Self {
            store_dir_path: config.store_dir_path.to_path_buf(),
            tmp_dir: TempDir::new(TMP_DIR_PREFIX).map(Arc::new)?,
            collection: PhantomData,
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
impl<C> Store for FsStore<C>
where
    C: Collection,
{
    async fn fetch(&self, key: &Key) -> Option<Block> {
        let path = format!(
            "{}/{}",
            self.store_dir_path.to_string_lossy(),
            Hash::from(key.0).to_hex()
        );
        fs::read(path).await.ok()
    }

    // TODO: This should perhaps return an error.
    async fn insert(&mut self, key: Key, block: Block) {
        let filename = format!("{}", Hash::from(key.0).to_hex());
        let path = self.tmp_dir.path().join(filename);
        if let Ok(mut tmp_file) = File::create(&path).await {
            if tmp_file.write_all(block.as_ref()).await.is_err() {
                return;
            }

            // TODO: Is this needed before calling rename?
            if tmp_file.sync_all().await.is_err() {
                return;
            }

            let store_path = format!(
                "{}/{}",
                self.store_dir_path.to_string_lossy(),
                Hash::from(key.0).to_hex()
            );

            if fs::rename(path, store_path).await.is_err() {
                return;
            }
        }
    }
}
