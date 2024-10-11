use std::path::PathBuf;

use bytes::BytesMut;
use tokio::io;

use super::*;
use crate::bucket::{errors, Bucket};
use crate::hasher::b3::MAX_BLOCK_SIZE_IN_BYTES;
use crate::hasher::byte_hasher::BlockHasher;
use crate::hasher::collector::BufCollector;
use crate::stream::verifier::{IncrementalVerifier, WithHashTreeCollector};

pub struct UntrustedFileWriterState {
    inner_state: InnerWriterState,
    current_hasher: BlockHasher,
}
impl UntrustedFileWriterState {
    pub(crate) async fn new(bucket: &Bucket) -> Result<Self, io::Error> {
        let mut block_hasher = BlockHasher::new();
        block_hasher.set_block(0);
        InnerWriterState::new(bucket).await.map(|inner_state| Self {
            inner_state,
            current_hasher: block_hasher,
        })
    }
}

impl WriterState for UntrustedFileWriterState {
    type Collector = IncrementalVerifier<WithHashTreeCollector<BufCollector>>;

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
        collector: &mut IncrementalVerifier<WithHashTreeCollector<BufCollector>>,
        bytes_before_remaining: &[u8],
        bytes: &[u8],
    ) -> Result<(), errors::WriteError> {
        if let Some(ref mut block) = self.inner_state.current_block_file {
            self.current_hasher.update(bytes_before_remaining);
            let block_hash = self.current_hasher.clone().finalize(false);
            collector
                .verify_hash(block_hash)
                .map_err(|_| errors::WriteError::InvalidBlockHash)?;
            // Add the block hash and the block file path to the list of block files so far
            // committed.
            self.inner_state.new_block(block_hash).await?;
            self.current_hasher = BlockHasher::new();
            self.current_hasher.set_block(self.inner_state.count_block);

            // Create a new file to write the remaining bytes.
            self.inner_state.create_new_block_file(bytes).await?;
            self.current_hasher.update(bytes);
        } else {
            return Err(errors::WriteError::BlockFileNotFound);
        }
        Ok(())
    }

    async fn continue_write(&mut self, bytes: &[u8]) -> Result<(), errors::WriteError> {
        self.inner_state.write_block_file(bytes).await?;
        self.current_hasher.update(bytes);
        Ok(())
    }

    async fn no_block_file(&mut self, bytes: &[u8]) -> Result<(), errors::WriteError> {
        self.inner_state.create_new_block_file(bytes).await?;
        self.current_hasher.update(bytes);
        Ok(())
    }

    async fn commit(
        mut self,
        mut collector: Self::Collector,
        root_hash: [u8; 32],
    ) -> Result<[u8; 32], errors::CommitError> {
        if let Some(ref mut block_file) = self.inner_state.current_block_file {
            let block_hash = self.current_hasher.clone().finalize(false);
            collector
                .verify_hash(block_hash)
                .map_err(|_| errors::CommitError::InvalidBlockHash)?;

            self.inner_state.new_block(block_hash).await?;
        }

        let mut finalize = collector.finalize();

        finalize
            .write_hash(&mut self.inner_state.header_file.file)
            .await?;

        self.inner_state.commit(&root_hash).await?;

        Ok(root_hash)
    }

    async fn rollback(self) -> Result<(), io::Error> {
        tokio::fs::remove_dir_all(self.inner_state.temp_file_path).await
    }
}
