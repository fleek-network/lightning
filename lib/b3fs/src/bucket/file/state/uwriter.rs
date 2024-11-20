use std::cell::RefCell;
use std::path::PathBuf;

use bytes::BytesMut;
use tokio::io;

use super::*;
use crate::bucket::{errors, Bucket};
use crate::hasher::b3::MAX_BLOCK_SIZE_IN_BYTES;
use crate::hasher::byte_hasher::BlockHasher;
use crate::hasher::collector::BufCollector;
use crate::stream::verifier::{IncrementalVerifier, WithHashTreeCollector};

/// Collector for untrusted file writing operations
pub(crate) struct UntrustedFileWriterCollector {
    /// The current block hasher
    current_hasher: BlockHasher,
    /// An incremental verifier for hash tree collection
    increment_verifier: RefCell<IncrementalVerifier<WithHashTreeCollector<BufCollector>>>,
    /// The root hash provided by the caller
    root_hash: [u8; 32],
}

impl WithCollector for UntrustedFileWriterCollector {
    /// In this case, we don't need to collect anything because the untrusted writer
    /// will write the bytes directly to the block file
    async fn collect(&mut self, bytes: &[u8]) -> Result<(), errors::WriteError> {
        Ok(())
    }

    /// In Untrusted case, we only need a new hasher for the new block. IncrementalVerifier remains
    /// the same.
    async fn on_new_block(
        &mut self,
        count_block: usize,
        _writer: impl AsyncWriteExt + Unpin,
    ) -> Result<(), io::Error> {
        self.current_hasher = BlockHasher::new();
        self.current_hasher.set_block(count_block);
        Ok(())
    }

    fn has_reach_block(&self, bytes_size: usize) -> bool {
        bytes_size >= MAX_BLOCK_SIZE_IN_BYTES
    }

    /// Update hasher and validate block hash against verifier when reaching the maximum block size
    async fn on_reach_full_block(
        &mut self,
        bytes: &[u8],
        _count_block: usize,
    ) -> Result<[u8; 32], errors::WriteError> {
        self.current_hasher.update(bytes);
        let block_hash = self.current_hasher.clone().finalize(false);
        self.increment_verifier
            .borrow_mut()
            .verify_hash(block_hash)?;
        Ok(block_hash)
    }

    /// Processes bytes after collection. In this case, we need to update hasher with the remaining
    /// bytes.
    async fn post_collect(&mut self, bytes: &[u8]) -> Result<(), errors::WriteError> {
        self.current_hasher.update(bytes);
        Ok(())
    }

    /// Get the final block hash and verify it against the verifier
    async fn final_block(
        &mut self,
        count_block: usize,
    ) -> Result<Option<[u8; 32]>, errors::WriteError> {
        let block_hash = self.current_hasher.clone().finalize(count_block == 0);
        self.increment_verifier
            .borrow_mut()
            .verify_hash(block_hash)?;
        Ok(Some(block_hash))
    }

    /// Finalizes the hash tree and return the root hash
    async fn finalize_tree(&mut self) -> Result<(BufCollector, [u8; 32]), errors::WriteError> {
        let mut collector = self.increment_verifier.take().finalize();
        Ok((collector, self.root_hash))
    }
}

/// Type alias for the untrusted file writer state
pub type UntrustedFileWriterState = InnerWriterState<UntrustedFileWriterCollector>;

impl UntrustedFileWriterCollector {
    /// Creates a new UntrustedFileWriterCollector
    pub(crate) fn new(root_hash: [u8; 32]) -> Self {
        let current_hasher = BlockHasher::new();
        let mut increment_verifier = IncrementalVerifier::default();
        increment_verifier.set_root_hash(root_hash);
        Self {
            current_hasher,
            increment_verifier: RefCell::new(increment_verifier),
            root_hash,
        }
    }

    /// Feeds a proof to the incremental verifier
    pub(crate) fn feed_proof(&mut self, proof: &[u8]) -> Result<(), errors::FeedProofError> {
        self.increment_verifier.borrow_mut().feed_proof(proof)?;
        Ok(())
    }
}
