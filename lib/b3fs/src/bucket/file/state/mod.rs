pub(crate) mod uwriter;
pub(crate) mod writer;

use std::cmp;
use std::os::unix::fs::FileExt;
use std::path::{Path, PathBuf};

use bytes::{Buf, BytesMut};
use rand::random;
use tokio::fs::{File, OpenOptions};
use tokio::io::{self, AsyncWriteExt, BufWriter};

use crate::bucket::{errors, Bucket};
use crate::hasher::b3::{BLOCK_SIZE_IN_CHUNKS, MAX_BLOCK_SIZE_IN_BYTES};
use crate::hasher::collector::BufCollector;
use crate::hasher::HashTreeCollector;
use crate::utils::{self, random_file_from};

pub struct BlockFile {
    size: usize,
    path: PathBuf,
    file: BufWriter<File>,
}

impl BlockFile {
    pub fn size(&self) -> usize {
        self.size
    }

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

    /// Write the bytes to the file and update the size of the file
    pub async fn write_all(&mut self, bytes: &[u8]) -> Result<(), io::Error> {
        self.file.write_all(bytes).await?;
        self.size += bytes.len();
        Ok(())
    }

    /// Flush the file
    pub async fn flush(&mut self) -> Result<(), io::Error> {
        self.file.flush().await
    }

    /// Commit the block file to the final path
    ///
    /// This mean the following:
    /// 1. Current block file is in temporary path "wal" directory with a random name
    /// 2. Move that file to the final path "blocks" directory with the block hash as the name
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

pub struct HeaderFile {
    pub path: PathBuf,
    pub file: BufWriter<File>,
}

impl HeaderFile {
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
        let version: u32 = 1;
        let num_entries: u32 = 0;
        file.write_u32_le(version).await?;
        file.write_u32_le(num_entries).await?;

        Ok(Self { path, file })
    }

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

    async fn update_num_entries(self, num_entries: u32) -> Result<(), io::Error> {
        let mut file = self.file.into_inner().try_into_std().unwrap();
        let task = tokio::task::spawn_blocking(move || async move {
            let bytes: [u8; 4] = num_entries.to_le_bytes();
            file.write_at(&bytes, 4)
                .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
            Ok(()) as Result<(), io::Error>
        })
        .await?;

        task.await
    }
}

pub(crate) struct InnerWriterState<T> {
    pub bucket: Bucket,
    pub temp_file_path: PathBuf,
    pub block_files: Vec<([u8; 32], PathBuf)>,
    pub current_block_file: Option<BlockFile>,
    pub header_file: HeaderFile,
    pub count_block: usize,
    pub collector: T,
}

impl<T: WithCollector> InnerWriterState<T> {
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

    async fn process_block(&mut self, bytes: &mut BytesMut) -> Result<(), errors::WriteError> {
        if let Some(ref mut block) = self.current_block_file {
            if block.size() + bytes.len() > MAX_BLOCK_SIZE_IN_BYTES {
                // Write `block_before_remaining` bytes to the current block file and create a
                // ew block file where we will write the remaining bytes which is in `bytes`.
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

    async fn write_block_file(&mut self, bytes: &[u8]) -> Result<(), io::Error> {
        if let Some(ref mut block) = self.current_block_file {
            block.write_all(bytes).await?;
        } else {
            return Err(io::Error::new(io::ErrorKind::Other, "Block file not found"));
        }
        Ok(())
    }

    async fn create_new_block_file(&mut self, bytes: &[u8]) -> Result<(), io::Error> {
        let mut block_file = BlockFile::from_wal_path(&self.temp_file_path).await?;
        self.current_block_file = Some(block_file);
        Ok(())
    }

    async fn flush(mut self, root_hash: &[u8; 32]) -> Result<(), io::Error> {
        self.header_file
            .flush(&self.bucket, &self.block_files, root_hash)
            .await?;
        tokio::fs::remove_dir_all(self.temp_file_path).await?;
        Ok(())
    }
}

pub trait WithCollector {
    async fn collect(&mut self, bytes: &[u8]) -> Result<(), errors::WriteError>;

    async fn reach_max_block(
        &mut self,
        bytes: &[u8],
        count_block: usize,
    ) -> Result<[u8; 32], errors::WriteError>;

    async fn on_new_block(
        &mut self,
        count_block: usize,
        writer: impl AsyncWriteExt + Unpin,
    ) -> Result<(), io::Error>;

    async fn post_collect(&mut self, bytes: &[u8]) -> Result<(), errors::WriteError>;

    async fn final_block(
        &mut self,
        count_block: usize,
    ) -> Result<Option<[u8; 32]>, errors::WriteError>;

    async fn finalize_tree(&mut self) -> Result<(BufCollector, [u8; 32]), errors::WriteError>;
}

pub trait WriterState {
    async fn write(&mut self, bytes: &[u8]) -> Result<(), errors::WriteError>;
    async fn commit(self) -> Result<[u8; 32], errors::CommitError>;
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
