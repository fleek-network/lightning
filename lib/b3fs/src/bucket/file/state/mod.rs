/// Module for untrusted writer functionality
pub(crate) mod uwriter;
/// Module for trusted writer functionality
pub(crate) mod writer;

use std::cmp;
use std::os::unix::fs::FileExt;
use std::path::{Path, PathBuf};

use bytes::{Buf, BytesMut};
use rand::random;
use tokio::fs::{File, OpenOptions};
use tokio::io::{self, AsyncWriteExt, BufWriter};

use crate::bucket::{
    errors,
    Bucket,
    HEADER_TYPE_FILE,
    HEADER_VERSION,
    POSITION_START_HASHES,
    POSITION_START_NUM_ENTRIES,
};
use crate::hasher::b3::{BLOCK_SIZE_IN_CHUNKS, MAX_BLOCK_SIZE_IN_BYTES};
use crate::hasher::collector::BufCollector;
use crate::hasher::HashTreeCollector;
use crate::utils::{self, random_file_from};

/// Represents a file containing a block of data
pub struct BlockFile {
    /// Size of the block file in bytes
    size: usize,
    /// Path to the block file
    path: PathBuf,
    /// Buffered writer for the file
    file: BufWriter<File>,
}

impl BlockFile {
    /// Returns the size of the block file
    pub fn size(&self) -> usize {
        self.size
    }

    /// Creates a new BlockFile from a given WAL (Write-Ahead Log) path
    pub async fn from_wal_path(path: &Path) -> Result<Self, io::Error> {
        let path = random_file_from(path);
        let file = BufWriter::new(
            OpenOptions::new()
                .create(true)
                .append(true)
                .open(path.clone())
                .await?,
        );
        Ok(Self {
            size: 0,
            path,
            file,
        })
    }

    /// Writes the given bytes to the file and updates the size
    pub async fn write_all(&mut self, bytes: &[u8]) -> Result<(), io::Error> {
        self.file.write_all(bytes).await?;
        self.size += bytes.len();
        Ok(())
    }

    /// Flushes the file, ensuring all buffered contents are written
    pub async fn flush(&mut self) -> Result<(), io::Error> {
        self.file.flush().await
    }

    /// Commits the block file to its final path
    ///
    /// This process involves:
    /// 1. The current block file is in a temporary "wal" directory with a random name
    /// 2. Moving that file to the final "blocks" directory with the block hash as the name
    pub async fn commit(
        &mut self,
        temp_path: &Path,
        block_hash: [u8; 32],
    ) -> Result<PathBuf, io::Error> {
        self.flush().await?;
        let new_block_file_path = temp_path.join(utils::to_hex(&block_hash).as_str());
        tokio::fs::rename(self.path.clone(), new_block_file_path.clone()).await?;
        Ok(new_block_file_path)
    }
}

/// Represents the header file of the B3FS filesystem
pub struct HeaderFile {
    /// Path to the header file
    pub path: PathBuf,
    /// Buffered writer for the file
    pub file: BufWriter<File>,
}

impl HeaderFile {
    /// Creates a new HeaderFile from a given WAL path
    pub(crate) async fn from_wal_path(path: &Path) -> Result<Self, io::Error> {
        let path = random_file_from(path);
        let mut file = BufWriter::new(
            OpenOptions::new()
                .create(true)
                .truncate(true)
                .write(true)
                .open(path.clone())
                .await?,
        );
        let num_entries: u32 = 0;
        file.write_u8(HEADER_TYPE_FILE).await?;
        file.write_u32_le(HEADER_VERSION).await?;
        file.write_u32_le(num_entries).await?;
        file.flush().await?;

        Ok(Self { path, file })
    }

    /// Flushes the header file and moves block files to their final locations
    pub(crate) async fn flush(
        mut self,
        bucket: &Bucket,
        block_files: &[([u8; 32], PathBuf)],
        root_hash: &[u8; 32],
    ) -> Result<(), io::Error> {
        let final_path = bucket.get_header_path(root_hash);
        let header_tmp_file_path = self.path.clone();
        self.update_num_entries(block_files.len() as u32).await?;

        tokio::fs::rename(header_tmp_file_path, final_path).await?;

        for (i, (hash, path)) in block_files.iter().enumerate() {
            let final_hash_file = bucket.get_block_path(i as u32, hash);
            tokio::fs::rename(path, final_hash_file).await?;
        }

        Ok(())
    }

    /// Updates the number of entries in the header file
    async fn update_num_entries(self, num_entries: u32) -> Result<(), io::Error> {
        let mut file = self.file.into_inner().try_into_std().unwrap();
        let task = tokio::task::spawn_blocking(move || async move {
            let bytes: [u8; 4] = num_entries.to_le_bytes();
            file.write_at(&bytes, POSITION_START_NUM_ENTRIES as u64)
                .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
            Ok(()) as Result<(), io::Error>
        })
        .await?;

        task.await
    }
}

/// Represents the internal state of a writer
pub(crate) struct InnerWriterState<T> {
    /// The bucket associated with this writer
    pub bucket: Bucket,
    /// Temporary file path for WAL
    pub temp_file_path: PathBuf,
    /// List of block files with their hashes and paths
    pub block_files: Vec<([u8; 32], PathBuf)>,
    /// The current block file being written to
    pub current_block_file: Option<BlockFile>,
    /// The header file for this write operation
    pub header_file: HeaderFile,
    /// The count of blocks written so far
    pub count_block: usize,
    /// The collector used for this write operation
    pub collector: T,
}

impl<T: WithCollector> InnerWriterState<T> {
    /// Creates a new InnerWriterState
    pub(crate) async fn new(bucket: &Bucket, collector: T) -> Result<Self, io::Error> {
        let wal_path = bucket.get_new_wal_path();
        tokio::fs::create_dir_all(&wal_path).await?;
        let header_file = HeaderFile::from_wal_path(&wal_path).await?;
        Ok(Self {
            bucket: bucket.clone(),
            temp_file_path: wal_path,
            count_block: 0,
            block_files: Vec::new(),
            current_block_file: None,
            header_file,
            collector,
        })
    }

    /// Processes a block of data, creating new block files as necessary
    async fn process_block(&mut self, bytes: &mut BytesMut) -> Result<(), errors::WriteError> {
        if let Some(ref mut block) = self.current_block_file {
            if block.size() + bytes.len() > MAX_BLOCK_SIZE_IN_BYTES {
                // Write `block_before_remaining` bytes to the current block file and create a
                // new block file where we will write the remaining bytes which is in `bytes`.
                let remaining = MAX_BLOCK_SIZE_IN_BYTES - block.size();
                let block_before_remaining = bytes.split_to(remaining);
                block.write_all(&block_before_remaining).await?;

                let block_hash = self
                    .collector
                    .reach_max_block(&block_before_remaining, self.count_block)
                    .await?;

                // Add the block hash and the block file path to the list of block files so far
                // committed.
                self.new_block(block_hash).await?;
                self.collector
                    .on_new_block(self.count_block, &mut self.header_file.file)
                    .await?;

                self.create_new_block_file(bytes).await?;
            }
        } else {
            self.create_new_block_file(bytes).await?;
        }
        Ok(())
    }

    /// Creates a new block and updates the state
    async fn new_block(&mut self, block_hash: [u8; 32]) -> Result<(), io::Error> {
        if let Some(ref mut block) = self.current_block_file {
            let new_block_file_path = block.commit(&self.temp_file_path, block_hash).await?;
            self.block_files.push((block_hash, new_block_file_path));
            self.count_block += 1;
        } else {
            return Err(io::Error::new(io::ErrorKind::Other, "Block file not found"));
        }
        Ok(())
    }

    /// Writes data to the current block file
    async fn write_block_file(&mut self, bytes: &[u8]) -> Result<(), io::Error> {
        if let Some(ref mut block) = self.current_block_file {
            block.write_all(bytes).await?;
        } else {
            return Err(io::Error::new(io::ErrorKind::Other, "Block file not found"));
        }
        Ok(())
    }

    /// Creates a new block file
    async fn create_new_block_file(&mut self, bytes: &[u8]) -> Result<(), io::Error> {
        let mut block_file = BlockFile::from_wal_path(&self.temp_file_path).await?;
        self.current_block_file = Some(block_file);
        Ok(())
    }

    /// Flushes all data to disk and finalizes the write operation
    async fn flush(mut self, root_hash: &[u8; 32]) -> Result<(), io::Error> {
        self.header_file
            .flush(&self.bucket, &self.block_files, root_hash)
            .await?;
        tokio::fs::remove_dir_all(self.temp_file_path).await?;
        Ok(())
    }
}

/// Trait for collectors used in the write process
pub trait WithCollector {
    /// Collects bytes during the write process
    async fn collect(&mut self, bytes: &[u8]) -> Result<(), errors::WriteError>;

    /// Called when a block reaches its maximum size
    async fn reach_max_block(
        &mut self,
        bytes: &[u8],
        count_block: usize,
    ) -> Result<[u8; 32], errors::WriteError>;

    /// Called when a new block is created
    async fn on_new_block(
        &mut self,
        count_block: usize,
        writer: impl AsyncWriteExt + Unpin,
    ) -> Result<(), io::Error>;

    /// Called after processing the bytes and after check if the block has reached the max size.
    async fn post_collect(&mut self, bytes: &[u8]) -> Result<(), errors::WriteError>;

    /// Called for the final block in the write process
    async fn final_block(
        &mut self,
        count_block: usize,
    ) -> Result<Option<[u8; 32]>, errors::WriteError>;

    /// Finalizes the hash tree
    async fn finalize_tree(&mut self) -> Result<(BufCollector, [u8; 32]), errors::WriteError>;
}

/// Trait defining the behavior of a writer state
pub trait WriterState {
    /// Writes bytes to the state
    async fn write(&mut self, bytes: &[u8]) -> Result<(), errors::WriteError>;
    /// Commits the write operation
    async fn commit(self) -> Result<[u8; 32], errors::CommitError>;
    /// Rolls back the write operation
    async fn rollback(self) -> Result<(), io::Error>;
}

impl<T: WithCollector> WriterState for InnerWriterState<T> {
    async fn write(&mut self, bytes: &[u8]) -> Result<(), errors::WriteError> {
        // Wrap the bytes in a BytesMut so we can split it into chunks.
        let mut bytes_mut = BytesMut::from(bytes);
        // Write the bytes to the current random block file incrementally or create a new block file
        // if the current block file is full.
        while bytes_mut.has_remaining() {
            let want = cmp::min(BLOCK_SIZE_IN_CHUNKS, bytes_mut.len());
            let mut bytes = bytes_mut.split_to(want);
            // Collect the bytes into the collector.
            self.collector.collect(&bytes).await?;
            // Process the block file to see if we reached the max block size.
            self.process_block(&mut bytes).await?;
            // Write the bytes to the block file.
            self.write_block_file(&bytes).await?;
            // Post collect the bytes into the collector.
            self.collector.post_collect(&bytes).await?;
        }

        Ok(())
    }

    /// Finalize this write and flush the data to the disk.
    async fn commit(mut self) -> Result<[u8; 32], errors::CommitError> {
        if self.current_block_file.is_some() {
            if let Some(block_hash) = self.collector.final_block(self.count_block).await? {
                self.new_block(block_hash).await?;
            }
        }

        let (mut final_collector, root_hash) = self.collector.finalize_tree().await?;

        if self.current_block_file.is_some() && self.count_block == 0 {
            // If we reached here, it is because we haven't fill up even 1 block, so we
            // cannot generate a block_hash for that block, because the block is not
            // completed. So we need to create that last block with the root_hash.
            // This is tested in
            // `bucket::file::writer::tests::*`
            self.new_block(root_hash).await?;
        }

        final_collector
            .write_hash(&mut self.header_file.file)
            .await?;

        self.flush(&root_hash).await?;

        Ok(root_hash)
    }

    /// Cancel this write and remove anything that this writer wrote to the disk.
    async fn rollback(self) -> Result<(), io::Error> {
        tokio::fs::remove_dir_all(self.temp_file_path).await
    }
}
