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

pub(crate) struct FileWriterCollector {
    hasher: Arc<RwLock<Blake3Hasher<BufCollector>>>,
    finalized_tree: Option<(BufCollector, [u8; 32])>,
}

impl WithCollector for FileWriterCollector {
    async fn collect(&mut self, bytes: &[u8]) -> Result<(), errors::WriteError> {
        self.hasher.write().await.update(bytes);
        Ok(())
    }
    async fn reach_max_block(
        &mut self,
        bytes: &[u8],
        count_block: usize,
    ) -> Result<[u8; 32], errors::WriteError> {
        self.hasher
            .read()
            .await
            .get_tree()
            .get_block_hash(count_block)
            .ok_or(errors::WriteError::BlockHashNotFound)
    }

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

    async fn post_collect(&mut self, bytes: &[u8]) -> Result<(), errors::WriteError> {
        Ok(())
    }

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
    async fn finalize_tree(&mut self) -> Result<(BufCollector, [u8; 32]), errors::WriteError> {
        if let Some(finalized_tree) = self.finalized_tree.take() {
            Ok(finalized_tree)
        } else {
            Ok(self.hasher.read().await.clone().finalize_tree())
        }
    }
}

impl FileWriterCollector {
    pub(crate) fn new() -> Self {
        Self {
            hasher: Arc::new(RwLock::new(Blake3Hasher::default())),
            finalized_tree: None,
        }
    }
}

pub type FileWriterState = InnerWriterState<FileWriterCollector>;
