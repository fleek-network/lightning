use std::path::PathBuf;
use std::sync::Arc;
use std::{cmp, io};

use bytes::{Buf as _, BufMut, BytesMut};
use fleek_blake3::tree::HashTreeBuilder;
use rand::random;
use tokio::fs::{File, OpenOptions};
use tokio::io::{AsyncReadExt, AsyncSeekExt as _, AsyncWriteExt, BufWriter};
use tokio::sync::RwLock;

use super::state::writer::{FileWriterCollector, FileWriterState};
use super::state::{InnerWriterState, WriterState};
use crate::bucket::{errors, Bucket};
use crate::hasher::b3::{BLOCK_SIZE_IN_CHUNKS, MAX_BLOCK_SIZE_IN_BYTES};
use crate::hasher::byte_hasher::Blake3Hasher;
use crate::hasher::collector::BufCollector;
use crate::hasher::HashTreeCollector;
use crate::utils::{self, tree_index};

pub struct FileWriter {
    state: FileWriterState,
}

impl FileWriter {
    pub async fn new(bucket: &Bucket) -> Result<Self, errors::WriteError> {
        let s = InnerWriterState::new(bucket, FileWriterCollector::new())
            .await
            .map(|state| Self { state })?;
        Ok(s)
    }

    pub async fn write(&mut self, bytes: &[u8]) -> Result<(), errors::WriteError> {
        self.state.write(bytes, false).await
    }

    /// Finalize this write and flush the data to the disk.
    pub async fn commit(mut self) -> Result<[u8; 32], errors::CommitError> {
        self.state.commit().await
    }

    /// Cancel this write and remove anything that this writer wrote to the disk.
    pub async fn rollback(self) -> Result<(), io::Error> {
        self.state.rollback().await
    }
}

#[cfg(test)]
mod tests {
    use std::env::temp_dir;

    use rand::random;
    use tokio::fs;

    use super::*;
    use crate::bucket::file::tests::verify_writer;
    use crate::bucket::file::B3FSFile;
    use crate::bucket::tests::get_random_file;

    async fn writer_with(test_name: &str, n_blocks: usize, additional_bytes: usize) {
        let temp_dir_name = random::<[u8; 32]>();
        let temp_dir = temp_dir().join(format!("{}_{}", test_name, utils::to_hex(&temp_dir_name)));
        let bucket = Bucket::open(&temp_dir).await.unwrap();
        let mut writer = FileWriter::new(&bucket).await.unwrap();
        let mut data = get_random_file(8192 * n_blocks);
        let mut number_blocks = n_blocks;
        for _ in 0..additional_bytes {
            data.push(random());
        }
        if additional_bytes > 0 {
            number_blocks += 1;
        }
        writer.write(&data).await.unwrap();
        writer.commit().await.unwrap();
        verify_writer(&temp_dir, number_blocks).await;
    }

    #[tokio::test]
    async fn test_writer_should_work_with_full_blocks() {
        let test_name = "test_writer_should_work_with_full_blocks";
        for i in 1..10 {
            let test_name = format!("{}_{}", test_name, i);
            writer_with(&test_name, i, 0).await;
        }
    }

    #[tokio::test]
    async fn test_writer_should_work_with_few_bytes() {
        let test_name = "test_writer_should_work_with_few_bytes";
        for i in 1..10 {
            let test_name = &format!("{}_{}", test_name, i);
            writer_with(test_name, 0, i * 20).await;
        }
    }

    #[tokio::test]
    async fn test_writer_should_work_with_one_byte() {
        let test_name = "test_writer_should_work_with_one_byte";
        writer_with(test_name, 0, 1).await;
    }

    #[tokio::test]
    async fn test_writer_should_work_with_full_blocks_and_few_more_bytes() {
        let test_name = "test_writer_should_work_with_full_blocks_and_few_more_bytes";
        for i in 1..10 {
            let test_name = &format!("{}_{}", test_name, i);
            writer_with(test_name, i, i * 20).await;
        }
    }
}
