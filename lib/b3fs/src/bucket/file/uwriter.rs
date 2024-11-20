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

    pub async fn write(&mut self, bytes: &[u8]) -> Result<(), errors::WriteError> {
        self.state.write(bytes).await
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

    use cmp::min;
    use rand::random;
    use tokio::fs;

    use super::*;
    use crate::bucket::file::tests::verify_writer;
    use crate::bucket::file::B3FSFile;
    use crate::bucket::tests::get_random_file;
    use crate::collections::{tree, HashTree};
    use crate::hasher::byte_hasher::Blake3Hasher;
    use crate::stream::walker::Mode;
    use crate::utils::{self, tree_index};

    #[tokio::test]
    async fn test_untrusted_file_writer_providing_incremental_proof() {
        for i in 1..10 {
            untrusted_file_writer_with(
                "test_untrusted_file_writer_providing_incremental_proof",
                i,
                0,
            );
        }
    }

    #[tokio::test]
    async fn test_untrusted_file_writer_providing_incremental_proof_few_bytes() {
        for i in 1..10 {
            untrusted_file_writer_with(
                "test_untrusted_file_writer_providing_incremental_proof_few_bytes",
                0,
                32 * i,
            );
        }
    }
    #[tokio::test]
    async fn test_untrusted_file_writer_providing_incremental_proof_three_blocks_plus_some_bytes() {
        for i in 1..10 {
            untrusted_file_writer_with(
                "test_untrusted_file_writer_providing_incremental_proof_three_blocks_plus_some_bytes",
                i,
                32 * i,
            );
        }
    }

    async fn untrusted_file_writer_with(
        test_name: &str,
        num_blocks: usize,
        additional_bytes_size: usize,
    ) {
        let mut n_blocks = num_blocks;
        let temp_dir_name = random::<[u8; 32]>();
        let temp_dir = temp_dir().join(format!("{}_{}", test_name, utils::to_hex(&temp_dir_name)));
        let bucket = Bucket::open(&temp_dir).await.unwrap();
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
        let mut init_idx = 0;
        let mut final_idx = 0;
        for i in 0..n_blocks {
            let index = i;
            let proof = hashtree.generate_proof(Mode::from_is_initial(index == 0), index);
            writer.feed_proof(proof.as_slice()).await.unwrap();
            let block_size = 8192 * 32;
            final_idx = min(init_idx + block_size, block.len());
            writer.write(&block[init_idx..final_idx]).await.unwrap();
            init_idx = final_idx;
        }
        let proof = writer.commit().await.unwrap();

        verify_writer(&temp_dir, n_blocks).await;
    }
}
