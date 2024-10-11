pub(crate) mod uwriter;
pub(crate) mod writer;

use std::os::unix::fs::FileExt;
use std::path::{Path, PathBuf};

use bytes::BytesMut;
use rand::random;
use tokio::fs::{File, OpenOptions};
use tokio::io::{self, AsyncWriteExt as _, BufWriter};

use crate::bucket::{errors, Bucket};
use crate::hasher::b3::MAX_BLOCK_SIZE_IN_BYTES;
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

pub(crate) struct InnerWriterState {
    pub bucket: Bucket,
    pub temp_file_path: PathBuf,
    pub block_files: Vec<([u8; 32], PathBuf)>,
    pub current_block_file: Option<BlockFile>,
    pub header_file: HeaderFile,
    pub count_block: usize,
}

impl InnerWriterState {
    pub(crate) async fn new(bucket: &Bucket) -> Result<Self, io::Error> {
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
        })
    }

    pub(crate) async fn new_block(&mut self, block_hash: [u8; 32]) -> Result<(), io::Error> {
        if let Some(ref mut block) = self.current_block_file {
            let new_block_file_path = block.commit(&self.temp_file_path, block_hash).await?;
            self.block_files.push((block_hash, new_block_file_path));
            self.count_block += 1;
        } else {
            return Err(io::Error::new(io::ErrorKind::Other, "Block file not found"));
        }
        Ok(())
    }

    pub(crate) async fn write_block_file(&mut self, bytes: &[u8]) -> Result<(), io::Error> {
        if let Some(ref mut block) = self.current_block_file {
            block.write_all(bytes).await?;
        } else {
            return Err(io::Error::new(io::ErrorKind::Other, "Block file not found"));
        }
        Ok(())
    }

    pub(crate) async fn create_new_block_file(&mut self, bytes: &[u8]) -> Result<(), io::Error> {
        let mut block_file = BlockFile::from_wal_path(&self.temp_file_path).await?;
        block_file.write_all(bytes).await?;
        self.current_block_file = Some(block_file);
        Ok(())
    }

    pub(crate) async fn commit(mut self, root_hash: &[u8; 32]) -> Result<(), io::Error> {
        self.header_file
            .flush(&self.bucket, &self.block_files, root_hash)
            .await?;
        tokio::fs::remove_dir_all(self.temp_file_path).await?;
        Ok(())
    }
}

pub trait WriterState {
    type Collector;

    fn get_mut_block_file(&mut self) -> &mut Option<BlockFile>;

    async fn reach_max_block(
        &mut self,
        collector: &mut Self::Collector,
        bytes_before_remaining: &[u8],
        bytes: &[u8],
    ) -> Result<(), errors::WriteError>;

    async fn continue_write(&mut self, bytes: &[u8]) -> Result<(), errors::WriteError>;

    async fn no_block_file(&mut self, bytes: &[u8]) -> Result<(), errors::WriteError>;

    async fn commit(
        self,
        collector: Self::Collector,
        root_hash: [u8; 32],
    ) -> Result<[u8; 32], errors::CommitError>;
    async fn rollback(self) -> Result<(), io::Error>;

    async fn next(
        &mut self,
        collector: &mut Self::Collector,
        bytes: &mut BytesMut,
    ) -> Result<(), errors::WriteError> {
        if let Some(ref mut block) = self.get_mut_block_file() {
            if block.size() + bytes.len() > MAX_BLOCK_SIZE_IN_BYTES {
                // Write `block_before_remaining` bytes to the current block file and create a
                // ew block file where we will write the remaining bytes which is in `bytes`.
                let remaining = MAX_BLOCK_SIZE_IN_BYTES - block.size();
                let block_before_remaining = bytes.split_to(remaining);
                block.write_all(&block_before_remaining).await?;

                self.reach_max_block(collector, &block_before_remaining, bytes)
                    .await?;
            } else {
                self.continue_write(bytes).await?;
            }
        } else {
            self.no_block_file(bytes).await?;
        }
        Ok(())
    }
}
