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

use crate::config::Config;
use crate::put::Putter;
use crate::store::Store;
use crate::{Block, BlockContent};

// #[derive(Clone)]
// pub struct BlockStore {}
//
// #[async_trait]
// impl<C: Collection> BlockStoreInterface<C> for BlockStore {
//     type SharedPointer<T: ?Sized + Send + Sync> = Arc<T>;
//
//     type Put = Putter<Self>;
//
//     fn init(config: Self::Config) -> anyhow::Result<Self> {
//         todo!()
//     }
//
//     async fn get_tree(&self, cid: &Blake3Hash) -> Option<Self::SharedPointer<Blake3Tree>> {
//         todo!()
//     }
//
//     async fn get(
//         &self,
//         block_counter: u32,
//         block_hash: &Blake3Hash,
//         compression: CompressionAlgoSet,
//     ) -> Option<Self::SharedPointer<ContentChunk>> { todo!()
//     }
//
//     fn put(&self, cid: Option<Blake3Hash>) -> Self::Put {
//         todo!()
//     }
//
//     fn get_root_dir(&self) -> PathBuf {
//         todo!()
//     }
// }
//
// impl ConfigConsumer for BlockStore {
//     const KEY: &'static str = "blockstore";
//     type Config = Config;
// }

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
pub struct Blockstore<C: Collection> {
    store_dir_path: PathBuf,
    tmp_dir: Arc<TempDir>,
    collection: PhantomData<C>,
}

impl<C: Collection> ConfigConsumer for Blockstore<C> {
    const KEY: &'static str = "fsstore";
    type Config = FsStoreConfig;
}

#[async_trait]
impl<C: Collection> BlockStoreInterface<C> for Blockstore<C> {
    type SharedPointer<T: ?Sized + Send + Sync> = Arc<T>;
    type Put = Putter<Self>;

    fn init(config: Self::Config) -> anyhow::Result<Self> {
        std::fs::create_dir_all(config.store_dir_path.clone())?;
        Ok(Self {
            store_dir_path: config.store_dir_path.to_path_buf(),
            tmp_dir: TempDir::new(TMP_DIR_PREFIX).map(Arc::new)?,
            collection: PhantomData,
        })
    }

    async fn get_tree(&self, cid: &Blake3Hash) -> Option<Self::SharedPointer<Blake3Tree>> {
        match bincode::deserialize::<BlockContent>(self.fetch(cid, None).await?.as_slice())
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
            self.fetch(block_hash, Some(block_counter as usize))
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
            Some(root) => Putter::verifier(self.clone(), root),
            None => Putter::trust(self.clone()),
        }
    }

    fn get_root_dir(&self) -> PathBuf {
        todo!()
    }
}

// TODO: Add logging.
#[async_trait]
impl<C> Store for Blockstore<C>
where
    C: Collection,
{
    async fn fetch(&self, key: &Blake3Hash, tag: Option<usize>) -> Option<Block> {
        let path = format!(
            "{}/{}",
            self.store_dir_path.to_string_lossy(),
            Hash::from(*key).to_hex()
        );
        fs::read(path).await.ok()
    }

    // TODO: This should perhaps return an error.
    async fn insert(
        &mut self,
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
                self.store_dir_path.to_string_lossy(),
                Hash::from(key).to_hex()
            );

            fs::rename(path, store_path).await?;
        }
        Ok(())
    }
}
