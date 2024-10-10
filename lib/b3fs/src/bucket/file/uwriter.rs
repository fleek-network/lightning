use std::ops::{Deref, DerefMut};
use std::path::PathBuf;
use std::{cmp, io};

use bytes::{Buf, BufMut, BytesMut};
use fleek_blake3::tree::HashTreeBuilder;
use tokio::fs::{File, OpenOptions};
use tokio::io::AsyncWriteExt;

use super::state::uwriter::UntrustedFileWriterState;
use super::state::WriterState as _;
use super::{writer, B3FSFile};
use crate::bucket::{errors, Bucket};
use crate::hasher::b3::{BLOCK_SIZE_IN_CHUNKS, MAX_BLOCK_SIZE_IN_BYTES};
use crate::hasher::byte_hasher::BlockHasher;
use crate::hasher::collector::BufCollector;
use crate::stream::verifier::{IncrementalVerifier, VerifierCollector, WithHashTreeCollector};
use crate::utils::to_hex;

pub struct UntrustedFileWriter {
    root_hash: [u8; 32],
    verifier: Box<IncrementalVerifier<WithHashTreeCollector<BufCollector>>>,
    state: UntrustedFileWriterState,
}

impl UntrustedFileWriter {
    pub async fn new(bucket: &Bucket, hash: [u8; 32]) -> Result<Self, errors::WriteError> {
        let mut verifier = IncrementalVerifier::default();
        verifier.set_root_hash(hash);
        let state = UntrustedFileWriterState::new(bucket).await?;
        Ok(Self {
            verifier: Box::new(verifier),
            state,
            root_hash: hash,
        })
    }

    pub async fn feed_proof(&mut self, proof: &[u8]) -> Result<(), errors::FeedProofError> {
        self.verifier
            .feed_proof(proof)
            .map_err(|_| errors::FeedProofError::InvalidProof)
    }

    pub async fn write(&mut self, bytes: &[u8]) -> Result<(), errors::WriteError> {
        // Wrap the bytes in a BytesMut so we can split it into chunks.
        let mut bytes_mut = BytesMut::from(bytes);
        // Write the bytes to the current random block file incrementally or create a new block file
        // if the current block file is full.
        while bytes_mut.has_remaining() {
            let want = cmp::min(BLOCK_SIZE_IN_CHUNKS, bytes_mut.len());
            let mut bytes = bytes_mut.split_to(want);
            //self.hasher.update(&bytes);
            self.state
                .next(self.verifier.deref_mut(), &mut bytes)
                .await?;
        }

        Ok(())
    }

    /// Finalize this write and flush the data to the disk.
    pub async fn commit(mut self) -> Result<[u8; 32], errors::CommitError> {
        self.state.commit(*self.verifier, self.root_hash).await
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
    use crate::collections::HashTree;
    use crate::hasher::byte_hasher::Blake3Hasher;
    use crate::stream::walker::Mode;
    use crate::utils;

    #[tokio::test]
    async fn test_untrusted_file_writer_providing_incremental_proof() {
        let temp_dir_name = random::<[u8; 32]>();
        let temp_dir = temp_dir().join(format!(
            "test_uwrite_should_work_{}",
            utils::to_hex(&temp_dir_name)
        ));
        let bucket = Bucket::open(&temp_dir).await.unwrap();
        let mut blake3_hasher: Blake3Hasher<Vec<[u8; 32]>> = Blake3Hasher::default();
        let block = get_random_file(8192 * 2);
        blake3_hasher.update(&block[..]);

        let (ref mut hashes, root) = blake3_hasher.finalize_tree();
        hashes.push(root);
        let hashtree = HashTree::try_from(&*hashes).unwrap();
        let mut writer = UntrustedFileWriter::new(&bucket, *hashtree.root())
            .await
            .unwrap();
        for i in 0..2 {
            let proof = hashtree.generate_proof(Mode::from_is_initial(i == 0), i);
            writer.feed_proof(proof.as_slice()).await.unwrap();
        }
        writer.write(&block).await.unwrap();
        let proof = writer.commit().await.unwrap();

        verify_writer(&temp_dir, 2).await;
    }
}
