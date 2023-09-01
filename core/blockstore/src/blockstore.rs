#![allow(unused)]

use std::io;
use std::marker::PhantomData;
use std::path::PathBuf;
use std::sync::Arc;

use async_trait::async_trait;
use blake3_tree::blake3::tree::{BlockHasher, HashTreeBuilder};
use blake3_tree::blake3::Hash;
use blake3_tree::IncrementalVerifier;
use bytes::{BufMut, BytesMut};
use lightning_interfaces::infu_collection::Collection;
use lightning_interfaces::types::{CompressionAlgoSet, CompressionAlgorithm};
use lightning_interfaces::{
    Blake3Hash,
    Blake3Tree,
    BlockStoreInterface,
    ConfigConsumer,
    ContentChunk,
    IncrementalPutInterface,
    PutFeedProofError,
    PutFinalizeError,
    PutWriteError,
};
use resolved_pathbuf::ResolvedPathBuf;
use serde::{Deserialize, Serialize};
use tempdir::TempDir;
use tokio::fs;
use tokio::fs::File;
use tokio::io::AsyncWriteExt;
use tokio::task::JoinSet;

use crate::config::{Config, BLOCK_DIR, INTERNAL_DIR};
use crate::put::Putter;
use crate::store::{Block, Store};
use crate::BlockContent;

pub const BLOCK_SIZE: usize = 256 << 10;

const TMP_DIR_PREFIX: &str = "tmp-store";

#[derive(Clone)]
pub struct Blockstore<C: Collection> {
    root: PathBuf,
    tmp_dir: Arc<TempDir>,
    collection: PhantomData<C>,
}

impl<C: Collection> ConfigConsumer for Blockstore<C> {
    const KEY: &'static str = "fsstore";
    type Config = Config;
}

#[async_trait]
impl<C: Collection> BlockStoreInterface<C> for Blockstore<C> {
    type SharedPointer<T: ?Sized + Send + Sync> = Arc<T>;
    type Put = Putter<Self>;

    fn init(config: Self::Config) -> anyhow::Result<Self> {
        std::fs::create_dir_all(config.root.clone())?;
        Ok(Self {
            root: config.root.to_path_buf(),
            tmp_dir: TempDir::new(TMP_DIR_PREFIX).map(Arc::new)?,
            collection: PhantomData,
        })
    }

    async fn get_tree(&self, cid: &Blake3Hash) -> Option<Self::SharedPointer<Blake3Tree>> {
        let encoded_tree: Vec<Blake3Hash> =
            bincode::deserialize(self.fetch(INTERNAL_DIR, cid, None).await?.as_slice())
                .expect("Tree to be properly serialized");
        Some(Arc::new(Blake3Tree(encoded_tree)))
    }

    async fn get(
        &self,
        block_counter: u32,
        block_hash: &Blake3Hash,
        _compression: CompressionAlgoSet,
    ) -> Option<Self::SharedPointer<ContentChunk>> {
        let block = self
            .fetch(BLOCK_DIR, block_hash, Some(block_counter as usize))
            .await?;
        Some(Arc::new(ContentChunk {
            compression: CompressionAlgorithm::Uncompressed,
            content: block,
        }))
    }

    fn put(&self, root: Option<Blake3Hash>) -> Self::Put {
        match root {
            Some(root) => Putter::verifier(self.clone(), root),
            None => Putter::trust(self.clone()),
        }
    }

    fn get_root_dir(&self) -> PathBuf {
        self.root.to_path_buf()
    }
}

// TODO: Add logging.
#[async_trait]
impl<C> Store for Blockstore<C>
where
    C: Collection,
{
    async fn fetch(&self, location: &str, key: &Blake3Hash, tag: Option<usize>) -> Option<Block> {
        let path = format!(
            "{}/{}",
            self.root.to_string_lossy(),
            Hash::from(*key).to_hex()
        );
        fs::read(path).await.ok()
    }

    async fn insert(
        &mut self,
        location: &str,
        key: Blake3Hash,
        block: Block,
        tag: Option<usize>,
    ) -> io::Result<()> {
        let filename = format!("{}", Hash::from(key).to_hex());
        let path = self.tmp_dir.path().join(filename);
        if let Ok(mut tmp_file) = File::create(&path).await {
            tmp_file.write_all(block.as_ref()).await?;

            // TODO: Is this needed before calling rename?
            tmp_file.sync_all().await?;

            let store_path = format!(
                "{}/{}",
                self.root.to_string_lossy(),
                Hash::from(key).to_hex()
            );

            fs::rename(path, store_path).await?;
        }
        Ok(())
    }
}
