#![allow(unused)]

use std::future::Future;
use std::io;
use std::marker::PhantomData;
use std::path::PathBuf;
use std::sync::{Arc, OnceLock};

use b3fs::bucket::dir::writer::DirWriter;
use b3fs::bucket::file::writer::FileWriter;
use b3fs::bucket::{Bucket, ContentHeader};
use blake3_tree::blake3::tree::{BlockHasher, HashTreeBuilder};
use blake3_tree::blake3::Hash;
use blake3_tree::utils::{HashTree, HashVec};
use blake3_tree::IncrementalVerifier;
use bytes::{BufMut, BytesMut};
use lightning_interfaces::prelude::*;
use lightning_interfaces::types::{Blake3Hash, CompressionAlgoSet, CompressionAlgorithm};
use parking_lot::RwLock;
use resolved_pathbuf::ResolvedPathBuf;
use serde::{Deserialize, Serialize};
use tokio::fs;
use tokio::fs::File;
use tokio::io::AsyncWriteExt;
use tokio::task::JoinSet;
use tracing::{error, trace};

use crate::config::Config;

pub const BLOCK_SIZE: usize = 256 << 10;

pub struct Blockstore<C: NodeComponents> {
    root: PathBuf,
    bucket: Bucket,
    indexer: Arc<OnceLock<C::IndexerInterface>>,
    _components: PhantomData<C>,
}

impl<C: NodeComponents> Clone for Blockstore<C> {
    fn clone(&self) -> Self {
        Self {
            root: self.root.clone(),
            bucket: self.bucket.clone(),
            indexer: self.indexer.clone(),
            _components: PhantomData,
        }
    }
}

impl<C: NodeComponents> ConfigConsumer for Blockstore<C> {
    const KEY: &'static str = "fsstore";
    type Config = Config;
}

impl<C: NodeComponents> BuildGraph for Blockstore<C> {
    fn build_graph() -> fdi::DependencyGraph {
        fdi::DependencyGraph::new().with(Self::new.wrap_with_block_on().with_event_handler(
            "_post",
            |mut this: fdi::RefMut<Self>,
             fdi::Cloned(indexer): fdi::Cloned<C::IndexerInterface>| {
                this.provide_indexer(indexer);
            },
        ))
    }
}

impl<C: NodeComponents> Blockstore<C> {
    fn new(
        config_provider: &C::ConfigProviderInterface,
    ) -> impl Future<Output = anyhow::Result<Self>> {
        let config = config_provider.get::<Self>();
        Self::init(config)
    }

    pub async fn init(config: Config) -> anyhow::Result<Self> {
        let root = config.root.to_path_buf();

        let bucket = Bucket::open(&root).await?;

        Ok(Self {
            bucket,
            root,
            indexer: Arc::new(OnceLock::new()),
            _components: PhantomData,
        })
    }

    /// Provide the blockstore with the indexer after initialization, this function
    /// should only be called once.
    pub fn provide_indexer(&mut self, indexer: C::IndexerInterface) {
        assert!(self.indexer.set(indexer).is_ok());
    }
}

impl<C: NodeComponents> BlockstoreInterface<C> for Blockstore<C> {
    fn get_bucket(&self) -> Bucket {
        self.bucket.clone()
    }

    fn get_root_dir(&self) -> PathBuf {
        self.root.to_path_buf()
    }
}

impl<C> Store for Blockstore<C>
where
    C: NodeComponents,
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
