#![allow(unused)]

use std::io;
use std::marker::PhantomData;
use std::path::PathBuf;
use std::sync::{Arc, OnceLock};

use blake3_tree::blake3::tree::{BlockHasher, HashTreeBuilder};
use blake3_tree::blake3::Hash;
use blake3_tree::utils::{HashTree, HashVec};
use blake3_tree::IncrementalVerifier;
use bytes::{BufMut, BytesMut};
use lightning_interfaces::infu_collection::Collection;
use lightning_interfaces::types::{Blake3Hash, CompressionAlgoSet, CompressionAlgorithm};
use lightning_interfaces::{
    BlockStoreInterface,
    ConfigConsumer,
    ContentChunk,
    IncrementalPutInterface,
    PutFeedProofError,
    PutFinalizeError,
    PutWriteError,
};
use parking_lot::RwLock;
use resolved_pathbuf::ResolvedPathBuf;
use serde::{Deserialize, Serialize};
use tempdir::TempDir;
use tokio::fs;
use tokio::fs::File;
use tokio::io::AsyncWriteExt;
use tokio::task::JoinSet;
use tracing::{error, trace};

use crate::config::{Config, BLOCK_DIR, INTERNAL_DIR, TMP_DIR};
use crate::put::Putter;
use crate::store::{Block, Store};

pub const BLOCK_SIZE: usize = 256 << 10;

pub struct Blockstore<C: Collection> {
    root: PathBuf,
    indexer: Arc<OnceLock<C::IndexerInterface>>,
    collection: PhantomData<C>,
}

impl<C: Collection> Clone for Blockstore<C> {
    fn clone(&self) -> Self {
        Self {
            root: self.root.clone(),
            indexer: self.indexer.clone(),
            collection: PhantomData,
        }
    }
}

impl<C: Collection> ConfigConsumer for Blockstore<C> {
    const KEY: &'static str = "fsstore";
    type Config = Config;
}

impl<C: Collection> BlockStoreInterface<C> for Blockstore<C> {
    type SharedPointer<T: ?Sized + Send + Sync> = Arc<T>;
    type Put = Putter<Self, C>;
    type DirPut = infusion::Blank<()>;

    fn init(config: Self::Config) -> anyhow::Result<Self> {
        let root = config.root.to_path_buf();
        let internal_dir = root.join(INTERNAL_DIR);
        let block_dir = root.join(BLOCK_DIR);
        let tmp_dir = root.join(TMP_DIR);

        std::fs::create_dir_all(&root)?;
        std::fs::create_dir_all(internal_dir)?;
        std::fs::create_dir_all(block_dir)?;
        std::fs::create_dir_all(tmp_dir)?;

        Ok(Self {
            root,
            indexer: Arc::new(OnceLock::new()),
            collection: PhantomData,
        })
    }

    /// Provide the blockstore with the indexer after initialization, this function
    /// should only be called once.
    fn provide_indexer(&mut self, indexer: C::IndexerInterface) {
        assert!(self.indexer.set(indexer).is_ok());
    }

    async fn get_tree(&self, cid: &Blake3Hash) -> Option<Self::SharedPointer<HashTree>> {
        let data = self.fetch(INTERNAL_DIR, cid, None).await?;
        if data.len() & 31 != 0 {
            error!("Tried to read corrupted proof from disk");
            return None;
        }

        Some(Arc::new(HashTree::from_inner(HashVec::from_inner(
            data.into_boxed_slice(),
        ))))
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
            Some(root) => Putter::verifier(
                self.clone(),
                root,
                self.indexer
                    .get()
                    .cloned()
                    .expect("Indexer to have been set"),
            ),
            None => Putter::trust(
                self.clone(),
                self.indexer
                    .get()
                    .cloned()
                    .expect("Indexer to have been set"),
            ),
        }
    }

    fn put_dir(&self, root: Option<Blake3Hash>) -> Self::DirPut {
        todo!()
    }

    fn get_root_dir(&self) -> PathBuf {
        self.root.to_path_buf()
    }
}

impl<C> Store for Blockstore<C>
where
    C: Collection,
{
    async fn fetch(&self, location: &str, key: &Blake3Hash, tag: Option<usize>) -> Option<Block> {
        let filename = match tag {
            Some(tag) => format!("{tag}-{}", Hash::from(*key).to_hex()),
            None => format!("{}", Hash::from(*key).to_hex()),
        };
        let path = self.root.to_path_buf().join(location).join(filename);
        trace!("Fetch {path:?}");
        fs::read(path).await.ok()
    }

    async fn insert(
        &mut self,
        location: &str,
        key: Blake3Hash,
        block: &[u8],
        tag: Option<usize>,
    ) -> io::Result<()> {
        let filename = match tag {
            Some(tag) => format!("{tag}-{}", Hash::from(key).to_hex()),
            None => format!("{}", Hash::from(key).to_hex()),
        };
        let tmp_file_name = format!("{}-{}", rand::random::<u64>(), filename);
        let tmp_file_path = self.root.to_path_buf().join(TMP_DIR).join(&tmp_file_name);
        if let Ok(mut tmp_file) = File::create(&tmp_file_path).await {
            tmp_file.write_all(block).await?;

            // TODO: Is this needed before calling rename?
            tmp_file.sync_all().await?;

            let store_path = self.root.to_path_buf().join(location).join(filename);

            trace!("Inserting {store_path:?}");

            fs::rename(tmp_file_path, store_path).await?;
        }
        Ok(())
    }
}
