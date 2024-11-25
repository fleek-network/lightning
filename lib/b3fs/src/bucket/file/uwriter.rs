use std::ops::{Deref, DerefMut};
use std::path::PathBuf;
use std::{cmp, io};

use bytes::{Buf, BufMut, BytesMut};
use fleek_blake3::tree::HashTreeBuilder;
use tokio::fs::{File, OpenOptions};
use tokio::io::AsyncWriteExt;

use super::state::uwriter::{UntrustedFileWriterCollector, UntrustedFileWriterState};
use super::state::{InnerWriterState, WriterState};
use super::{writer, B3FSFile};
use crate::bucket::{errors, Bucket};
use crate::hasher::b3::{BLOCK_SIZE_IN_CHUNKS, MAX_BLOCK_SIZE_IN_BYTES};
use crate::hasher::byte_hasher::BlockHasher;
use crate::hasher::collector::BufCollector;
use crate::stream::verifier::{IncrementalVerifier, VerifierCollector, WithHashTreeCollector};
use crate::utils::to_hex;

pub struct UntrustedFileWriter {
    state: UntrustedFileWriterState,
}

impl UntrustedFileWriter {
    pub async fn new(bucket: &Bucket, hash: [u8; 32]) -> Result<Self, errors::WriteError> {
        let state = UntrustedFileWriterCollector::new(hash);
        let s = InnerWriterState::new(bucket, state)
            .await
            .map(|state| Self { state })?;
        Ok(s)
    }

    pub async fn feed_proof(&mut self, proof: &[u8]) -> Result<(), errors::FeedProofError> {
        self.state.collector.feed_proof(proof)
    }

    pub async fn write(
        &mut self,
        bytes: &[u8],
        last_bytes: bool,
    ) -> Result<(), errors::WriteError> {
        self.state.write(bytes, last_bytes).await
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
    use core::num;
    use std::env::temp_dir;
    use std::fmt::format;

    use arrayvec::ArrayString;
    use cmp::min;
    use rand::random;
    use tokio::fs;
    use tokio::io::{AsyncReadExt, AsyncSeekExt};

    use self::writer::FileWriter;
    use super::*;
    use crate::bucket::file::reader::B3File;
    use crate::bucket::file::tests::verify_writer;
    use crate::bucket::file::B3FSFile;
    use crate::bucket::tests::get_random_file;
    use crate::bucket::POSITION_START_NUM_ENTRIES;
    use crate::collections::{tree, HashTree};
    use crate::hasher::byte_hasher::Blake3Hasher;
    use crate::stream::walker::Mode;
    use crate::utils::{self, from_hex, tree_index};

    async fn untrusted_file_writer_with(
        test_name: &str,
        num_blocks: usize,
        additional_bytes_size: usize,
    ) -> (PathBuf, usize) {
        let mut n_blocks = num_blocks;
        let temp_dir_name = random::<[u8; 32]>();
        let temp_dir_path =
            temp_dir().join(format!("{}_{}", test_name, utils::to_hex(&temp_dir_name)));
        let bucket = Bucket::open(&temp_dir_path).await.unwrap();
        let mut blake3_hasher: Blake3Hasher<Vec<[u8; 32]>> = Blake3Hasher::default();
        let mut block = get_random_file(n_blocks * 8192);
        for _ in 0..additional_bytes_size {
            block.push(random());
        }
        blake3_hasher.update(&block[..]);
        if additional_bytes_size > 0 {
            n_blocks += 1;
        }

        let (ref mut hashes, root) = blake3_hasher.finalize_tree();
        hashes.push(root);
        let hashtree = HashTree::try_from(&*hashes).unwrap();
        let mut writer = UntrustedFileWriter::new(&bucket, root).await.unwrap();
        let blocks = block.chunks(8192 * 32);
        for (i, chunk) in blocks.enumerate() {
            let index = i;
            let proof = hashtree.generate_proof(Mode::from_is_initial(index == 0), index);
            writer.feed_proof(proof.as_slice()).await.unwrap();
            writer.write(chunk, i == n_blocks - 1).await.unwrap();
        }
        let proof = writer.commit().await.unwrap();

        (temp_dir_path, n_blocks)
    }

    async fn untrusted_writer_then_async_reader_with_proof(
        test_name: &str,
        num_blocks: usize,
        additional_bytes_size: usize,
    ) {
        let (path, num_blocks) =
            untrusted_file_writer_with(test_name, num_blocks, additional_bytes_size).await;

        let mut dir_headers = tokio::fs::read_dir(path.join("headers")).await.unwrap();
        let file_reader = dir_headers.next_entry().await.unwrap().unwrap();
        let file_name = file_reader.file_name();
        let root_hash = ArrayString::from(file_name.to_str().unwrap()).unwrap();
        let root_hash = from_hex(&root_hash);
        let mut file_reader = tokio::fs::File::open(file_reader.path()).await.unwrap();

        let mut reader = B3File::new(num_blocks as u32, file_reader);
        let mut hash_tree = reader.hashtree().await.unwrap();

        let temp_dir_path = random();
        let temp_dir = temp_dir().join(format!(
            "{}_new_file_{}",
            test_name,
            utils::to_hex(&temp_dir_path)
        ));
        let bucket = Bucket::open(&temp_dir).await.unwrap();
        let previous_bucket = Bucket::open(&path).await.unwrap();
        let mut writer = UntrustedFileWriter::new(&bucket, root_hash).await.unwrap();
        for i in 0..num_blocks {
            let proof = hash_tree.generate_proof(i as u32).await.unwrap();
            let slice = proof.as_slice();
            writer.feed_proof(slice).await.unwrap();
            let hash_block = hash_tree.get_hash(i as u32).await.unwrap().unwrap();
            let block_path = previous_bucket.get_block_path(i as u32, &hash_block);
            let bytes = tokio::fs::read(block_path).await.unwrap();
            writer.write(&bytes, num_blocks - 1 == i).await.unwrap();
        }

        let hash_root = writer.commit().await.unwrap();

        assert_eq!(hash_root, root_hash);
        verify_writer(&path, num_blocks).await;
    }

    #[tokio::test]
    async fn test_untrusted_file_writer_providing_incremental_proof() {
        let test_name = "test_untrusted_file_writer_providing_incremental_proof";
        for i in 1..10 {
            let test_name = format!("{}_{}", test_name, i);
            let (path, n_blocks) = untrusted_file_writer_with(&test_name, i, 0).await;
            verify_writer(&path, n_blocks).await;
        }
    }

    #[tokio::test]
    async fn test_untrusted_file_writer_providing_incremental_proof_few_bytes() {
        let test_name = "test_untrusted_file_writer_providing_incremental_proof_few_bytes";
        for i in 1..10 {
            let test_name = format!("{}_{}", test_name, i);
            let (path, n_blocks) = untrusted_file_writer_with(&test_name, 0, 32 * i).await;
            verify_writer(&path, n_blocks).await;
        }
    }
    #[tokio::test]
    async fn test_untrusted_file_writer_providing_incremental_proof_some_blocks_plus_some_bytes() {
        let test_name =
            "test_untrusted_file_writer_providing_incremental_proof_three_blocks_plus_some_bytes";
        for i in 1..10 {
            let test_name = format!("{}_{}", test_name, i);
            let (path, n_blocks) = untrusted_file_writer_with(&test_name, i, 32 * i).await;
            verify_writer(&path, n_blocks).await;
        }
    }

    #[tokio::test]
    async fn test_untrusted_writer_then_async_reader_with_proof_full_blocks() {
        let test_name = "test_untrusted_writer_then_async_reader_with_proof_full_blocks";
        for i in 1..10 {
            let test_name = format!("{}_{}", test_name, i);
            untrusted_writer_then_async_reader_with_proof(&test_name, i, 0).await;
        }
    }

    #[tokio::test]
    async fn test_untrusted_writer_then_async_reader_with_proof_full_blocks_plus_some_bytes() {
        let test_name =
            "test_untrusted_writer_then_async_reader_with_proof_full_blocks_plus_some_bytes";
        for i in 1..10 {
            let test_name = format!("{}_{}", test_name, i);
            untrusted_writer_then_async_reader_with_proof(&test_name, i, i * 32).await;
        }
    }

    #[tokio::test]
    async fn test_untrusted_writer_then_async_reader_with_proof_non_full_blocks() {
        let test_name = "test_untrusted_writer_then_async_reader_with_proof_non_full_blocks";
        for i in 1..10 {
            let test_name = format!("{}_{}", test_name, i);
            untrusted_writer_then_async_reader_with_proof(&test_name, 0, i * 32).await;
        }
    }
}
