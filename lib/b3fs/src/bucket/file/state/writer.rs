use std::cell::LazyCell;
use std::mem::MaybeUninit;
use std::os::unix::fs::FileExt;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use bytes::BytesMut;
use rand::random;
use tokio::fs::{File, OpenOptions};
use tokio::io::{self, AsyncWriteExt as _, BufWriter};
use tokio::sync::RwLock;

use super::*;
use crate::bucket::{errors, Bucket};
use crate::hasher::b3::MAX_BLOCK_SIZE_IN_BYTES;
use crate::hasher::byte_hasher::Blake3Hasher;
use crate::hasher::collector::BufCollector;
use crate::hasher::HashTreeCollector;
use crate::utils;

/// Collector for trusted file writing operations
pub(crate) struct FileWriterCollector {
    /// The Blake3 hasher wrapped in an Arc<RwLock> for concurrent access
    hasher: Arc<RwLock<Blake3Hasher<BufCollector>>>,
    /// The finalized hash tree and root hash, if available.
    finalized_tree: Option<(BufCollector, [u8; 32])>,
}

impl WithCollector for FileWriterCollector {
    /// Collects bytes by updating the hasher.
    async fn collect(&mut self, bytes: &[u8]) -> Result<(), errors::WriteError> {
        self.hasher.write().await.update(bytes);
        Ok(())
    }

    /// Tursted Writer needs to wait for at least 1 more byte before cutting a block
    fn has_reach_block(&self, bytes_size: usize) -> bool {
        bytes_size > MAX_BLOCK_SIZE_IN_BYTES
    }

    /// Retrieves the block hash when a block reaches its maximum size
    async fn on_reach_full_block(
        &mut self,
        count_block: usize,
        _last_bytes: bool,
    ) -> Result<Option<[u8; 32]>, errors::WriteError> {
        Ok(self
            .hasher
            .read()
            .await
            .get_tree()
            .get_block_hash(count_block))
    }

    /// Writes the hash tree to the provided writer when a new block is created
    async fn on_new_block(
        &mut self,
        count_block: usize,
        writer: impl AsyncWriteExt + Unpin,
    ) -> Result<(), io::Error> {
        self.hasher
            .write()
            .await
            .get_tree_mut()
            .write_hash(writer)
            .await
    }

    /// Finalizes the last block and prepares the finalized tree
    async fn final_block(
        &mut self,
        count_block: usize,
    ) -> Result<Option<[u8; 32]>, errors::WriteError> {
        let (mut collector, root_hash) = self.hasher.read().await.clone().finalize_tree();
        let block_hash = collector.get_block_hash(count_block);
        collector.push(root_hash);
        self.finalized_tree = Some((collector, root_hash));
        Ok(block_hash)
    }

    /// Returns the finalized hash tree and root hash
    async fn finalize_tree(&mut self) -> Result<(BufCollector, [u8; 32]), errors::WriteError> {
        if let Some(finalized_tree) = self.finalized_tree.take() {
            Ok(finalized_tree)
        } else {
            Ok(self.hasher.read().await.clone().finalize_tree())
        }
    }
}

impl FileWriterCollector {
    /// Creates a new FileWriterCollector with a default Blake3Hasher
    pub(crate) fn new() -> Self {
        Self {
            hasher: Arc::new(RwLock::new(Blake3Hasher::default())),
            finalized_tree: None,
        }
    }
}

/// Type alias for the trusted file writer state
pub type FileWriterState = InnerWriterState<FileWriterCollector>;
