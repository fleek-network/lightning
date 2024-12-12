#![allow(unused)]

use std::fmt::Write;
use std::future::Future;
use std::io;
use std::marker::PhantomData;
use std::ops::{Deref, DerefMut};
use std::path::PathBuf;
use std::sync::{Arc, OnceLock};

use b3fs::bucket::dir::uwriter::UntrustedDirWriter;
use b3fs::bucket::dir::writer::DirWriter;
use b3fs::bucket::errors::{CommitError, FeedProofError, WriteError};
use b3fs::bucket::file::uwriter::UntrustedFileWriter;
use b3fs::bucket::file::writer::FileWriter;
use b3fs::bucket::{Bucket, ContentHeader};
use blake3_tree::blake3::tree::{BlockHasher, HashTreeBuilder};
use blake3_tree::blake3::Hash;
use blake3_tree::utils::{HashTree, HashVec};
use blake3_tree::IncrementalVerifier;
use bytes::{BufMut, BytesMut};
use lightning_interfaces::prelude::*;
use lightning_interfaces::types::{Blake3Hash, CompressionAlgoSet, CompressionAlgorithm};
use lightning_interfaces::{
    DirTrustedWriter,
    DirUntrustedWriter,
    FileTrustedWriter,
    FileUntrustedWriter,
    _IndexerInterface,
};
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

    type FileWriter = FWriter<C>;

    type UFileWriter = FUWriter<C>;

    type DirWriter = DWriter<C>;

    type UDirWriter = DUWriter<C>;

    async fn file_writer(&self) -> Result<Self::FileWriter, WriteError> {
        let fw = FileWriter::new(&self.get_bucket()).await?;
        Ok(FWriter {
            writer: fw,
            indexer: self.indexer.clone(),
        })
    }

    async fn file_untrusted_writer(
        &self,
        root_hash: Blake3Hash,
    ) -> Result<Self::UFileWriter, WriteError> {
        let fw = UntrustedFileWriter::new(&self.get_bucket(), root_hash).await?;
        Ok(FUWriter {
            writer: fw,
            indexer: self.indexer.clone(),
        })
    }

    async fn dir_writer(&self, num_entries: usize) -> Result<Self::DirWriter, WriteError> {
        let dw = DirWriter::new(&self.get_bucket(), num_entries).await?;
        Ok(DWriter {
            writer: dw,
            indexer: self.indexer.clone(),
        })
    }

    async fn dir_untrusted_writer(
        &self,
        root_hash: Blake3Hash,
        num_entries: usize,
    ) -> Result<Self::UDirWriter, WriteError> {
        let dw = UntrustedDirWriter::new(&self.get_bucket(), num_entries, root_hash).await?;
        Ok(DUWriter {
            writer: dw,
            indexer: self.indexer.clone(),
        })
    }
}

pub struct FUWriter<C: NodeComponents> {
    writer: UntrustedFileWriter,
    indexer: Arc<OnceLock<C::IndexerInterface>>,
}

impl<C: NodeComponents> FileUntrustedWriter for FUWriter<C> {
    async fn feed_proof(&mut self, proof: &[u8]) -> Result<(), FeedProofError> {
        self.writer.feed_proof(proof).await
    }
}

impl<C: NodeComponents> FileTrustedWriter for FUWriter<C> {
    async fn write(&mut self, content: &[u8], last_bytes: bool) -> Result<(), WriteError> {
        self.writer.write(content, last_bytes).await
    }

    async fn commit(self) -> Result<Blake3Hash, CommitError> {
        let hash = self.writer.commit().await?;
        let indexer = self.indexer.get().ok_or_else(|| CommitError::LockError)?;
        IndexerInterface::register(indexer, hash).await;
        Ok(hash)
    }

    async fn rollback(self) -> Result<(), io::Error> {
        self.writer.rollback().await
    }
}

pub struct FWriter<C: NodeComponents> {
    writer: FileWriter,
    indexer: Arc<OnceLock<C::IndexerInterface>>,
}

impl<C: NodeComponents> FileTrustedWriter for FWriter<C> {
    async fn write(&mut self, content: &[u8], _last_bytes: bool) -> Result<(), WriteError> {
        self.writer.write(content).await
    }

    async fn commit(self) -> Result<Blake3Hash, CommitError> {
        let hash = self.writer.commit().await?;
        let indexer = self.indexer.get().ok_or_else(|| CommitError::LockError)?;
        IndexerInterface::register(indexer, hash).await;
        Ok(hash)
    }

    async fn rollback(self) -> Result<(), io::Error> {
        self.writer.rollback().await
    }
}

pub struct DUWriter<C: NodeComponents> {
    writer: UntrustedDirWriter,
    indexer: Arc<OnceLock<C::IndexerInterface>>,
}

impl<C: NodeComponents> DirUntrustedWriter for DUWriter<C> {
    async fn feed_proof(&mut self, proof: &[u8]) -> Result<(), FeedProofError> {
        self.writer.feed_proof(proof).await
    }
}

impl<C: NodeComponents> DirTrustedWriter for DUWriter<C> {
    async fn insert(
        &mut self,
        entry: b3fs::entry::BorrowedEntry<'_>,
    ) -> Result<(), b3fs::bucket::errors::InsertError> {
        self.writer.insert(entry).await
    }

    async fn commit(self) -> Result<Blake3Hash, CommitError> {
        let hash = self.writer.commit().await?;
        let indexer = self.indexer.get().ok_or_else(|| CommitError::LockError)?;
        IndexerInterface::register(indexer, hash).await;
        Ok(hash)
    }

    async fn rollback(self) -> Result<(), io::Error> {
        self.writer.rollback().await
    }
}
pub struct DWriter<C: NodeComponents> {
    writer: DirWriter,
    indexer: Arc<OnceLock<C::IndexerInterface>>,
}

impl<C: NodeComponents> DirTrustedWriter for DWriter<C> {
    async fn insert(
        &mut self,
        entry: b3fs::entry::BorrowedEntry<'_>,
    ) -> Result<(), b3fs::bucket::errors::InsertError> {
        self.writer.insert(entry).await
    }

    async fn commit(self) -> Result<Blake3Hash, CommitError> {
        let hash = self.writer.commit().await?;
        let indexer = self.indexer.get().ok_or_else(|| CommitError::LockError)?;
        IndexerInterface::register(indexer, hash).await;
        Ok(hash)
    }

    async fn rollback(self) -> Result<(), io::Error> {
        self.writer.rollback().await
    }
}
