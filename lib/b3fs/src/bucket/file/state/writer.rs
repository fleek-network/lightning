use std::os::unix::fs::FileExt;
use std::path::{Path, PathBuf};

use bytes::BytesMut;
use rand::random;
use tokio::fs::{File, OpenOptions};
use tokio::io::{self, AsyncWriteExt as _, BufWriter};

use super::*;
use crate::bucket::{errors, Bucket};
use crate::hasher::b3::MAX_BLOCK_SIZE_IN_BYTES;
use crate::hasher::collector::BufCollector;
use crate::hasher::HashTreeCollector;
use crate::utils;

/// Final state should be usefull for both writers: trusted and untrusted
pub(crate) struct FileWriterState {
    inner_state: InnerWriterState,
}

impl WriterState for FileWriterState {
    type Collector = BufCollector;

    fn get_mut_block_file(&mut self) -> &mut Option<BlockFile> {
        &mut self.inner_state.current_block_file
    }

    /// This function is called when we have written `block_before_remaining` bytes to the current
    /// block or when there is no current block file.
    ///
    /// - In the first case, since we already have a block file, we ask to the collector for the
    ///   current block hash and we commit the block file to the final path. Then we create a new
    ///   block file and write the remaining bytes.
    ///
    ///
    /// - In the second case, we create a new block file and write the bytes to it.
    async fn reach_max_block(
        &mut self,
        collector: &mut BufCollector,
        block_before_remaining: &[u8],
        bytes: &[u8],
    ) -> Result<(), errors::WriteError> {
        if let Some(ref mut block) = self.inner_state.current_block_file {
            // Because we have written `block_before_remaining` bytes to the current block
            // file, and we have more bytes than a
            // `MAX_BLOCK_SIZE_IN_BYTES`, we know that a new hash has been created in the
            // tree. Therefore we can safely get the created block hash
            let block_hash = collector
                .get_block_hash(self.inner_state.count_block)
                .ok_or(errors::WriteError::BlockHashNotFound)?;

            // Add the block hash and the block file path to the list of block files so far
            // committed.
            self.inner_state.new_block(block_hash).await?;

            // Write the block hash to the header file.
            collector
                .write_hash(&mut self.inner_state.header_file.file)
                .await?;
            // Create a new file to write the remaining bytes.
            self.inner_state.create_new_block_file(bytes).await?;
        } else {
            return Err(errors::WriteError::BlockFileNotFound);
        }
        Ok(())
    }

    async fn continue_write(&mut self, bytes: &[u8]) -> Result<(), errors::WriteError> {
        self.inner_state.write_block_file(bytes).await?;
        Ok(())
    }

    async fn no_block_file(&mut self, bytes: &[u8]) -> Result<(), errors::WriteError> {
        self.inner_state.create_new_block_file(bytes).await?;
        Ok(())
    }

    /// Finalize the write, collect the hashes and commit the block and header files to the final
    /// path.
    async fn commit(
        mut self,
        mut collector: BufCollector,
        root_hash: [u8; 32],
    ) -> Result<[u8; 32], errors::CommitError> {
        if let Some(ref mut block_file) = self.inner_state.current_block_file {
            if let Some(block_hash) = collector.get_block_hash(self.inner_state.count_block) {
                self.inner_state.new_block(block_hash).await?;
            }
        }

        collector
            .write_hash(&mut self.inner_state.header_file.file)
            .await?;
        self.inner_state.commit(&root_hash).await?;

        Ok(root_hash)
    }

    /// Rollback the write and remove the temporary files.
    async fn rollback(self) -> Result<(), io::Error> {
        tokio::fs::remove_dir_all(self.inner_state.temp_file_path).await
    }
}

impl FileWriterState {
    pub(crate) async fn new(bucket: &Bucket) -> Result<Self, io::Error> {
        InnerWriterState::new(bucket)
            .await
            .map(|inner_state| Self { inner_state })
    }
}
