use std::path::PathBuf;
use std::{cmp, io};

use bytes::{Buf as _, BufMut, BytesMut};
use fleek_blake3::tree::HashTreeBuilder;
use rand::random;
use tokio::fs::{File, OpenOptions};
use tokio::io::{AsyncReadExt, AsyncSeekExt as _, AsyncWriteExt, BufWriter};

use super::state::writer::FileWriterState;
use super::state::WriterState;
use crate::bucket::{errors, Bucket};
use crate::hasher::b3::{BLOCK_SIZE_IN_CHUNKS, MAX_BLOCK_SIZE_IN_BYTES};
use crate::hasher::byte_hasher::Blake3Hasher;
use crate::hasher::collector::BufCollector;
use crate::hasher::HashTreeCollector;
use crate::utils::{self, tree_index};

pub struct FileWriter {
    hasher: Blake3Hasher<BufCollector>,
    state: FileWriterState,
}

impl FileWriter {
    pub async fn new(bucket: &Bucket) -> Result<Self, errors::WriteError> {
        Ok(Self {
            hasher: Blake3Hasher::default(),
            state: FileWriterState::new(bucket).await?,
        })
    }

    pub async fn write(&mut self, bytes: &[u8]) -> Result<(), errors::WriteError> {
        // Wrap the bytes in a BytesMut so we can split it into chunks.
        let mut bytes_mut = BytesMut::from(bytes);
        // Write the bytes to the current random block file incrementally or create a new block file
        // if the current block file is full.
        while bytes_mut.has_remaining() {
            let want = cmp::min(BLOCK_SIZE_IN_CHUNKS, bytes_mut.len());
            let mut bytes = bytes_mut.split_to(want);
            self.hasher.update(&bytes);
            self.state
                .next(self.hasher.get_tree_mut(), &mut bytes)
                .await?;
        }

        Ok(())
    }

    /// Finalize this write and flush the data to the disk.
    pub async fn commit(mut self) -> Result<[u8; 32], errors::CommitError> {
        let (mut collector, root_hash) = self.hasher.finalize_tree();
        // Force pushing the root hash to the collector.
        collector.push(root_hash);
        self.state.commit(collector, root_hash).await
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
    use crate::bucket::file::tests::{get_random_file, verify_writer};
    use crate::bucket::file::B3FSFile;

    #[tokio::test]
    async fn test_trusted_write_should_work_and_be_consistent_with_fs() {
        let temp_dir_name = random::<[u8; 32]>();
        let temp_dir = temp_dir().join(format!(
            "test_write_should_work_{}",
            utils::to_hex(&temp_dir_name)
        ));
        let bucket = Bucket::open(&temp_dir).await.unwrap();
        let mut writer = FileWriter::new(&bucket).await.unwrap();
        let data = get_random_file(8192 * 2);
        writer.write(&data).await.unwrap();
        writer.commit().await.unwrap();
        verify_writer(&temp_dir, 2).await;
    }

    #[tokio::test]
    async fn test_trusted_write_should_work_and_be_consistent_with_fs_more_blocks() {
        let temp_dir_name = random::<[u8; 32]>();
        let temp_dir = temp_dir().join(format!(
            "test_write_should_work_more_blocks_{}",
            utils::to_hex(&temp_dir_name)
        ));
        let bucket = Bucket::open(&temp_dir).await.unwrap();
        let mut writer = FileWriter::new(&bucket).await.unwrap();
        let data = get_random_file(32768);
        writer.write(&data).await.unwrap();
        writer.commit().await.unwrap();
        verify_writer(&temp_dir, 4).await;
    }
}
