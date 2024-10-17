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

pub(crate) struct UntrustedFileWriterCollector {
    current_hasher: BlockHasher,
    increment_verifier: RefCell<IncrementalVerifier<WithHashTreeCollector<BufCollector>>>,
    root_hash: [u8; 32],
}

impl WithCollector for UntrustedFileWriterCollector {
    async fn collect(&mut self, bytes: &[u8]) -> Result<(), errors::WriteError> {
        Ok(())
    }

    async fn on_new_block(
        &mut self,
        count_block: usize,
        _writer: impl AsyncWriteExt + Unpin,
    ) -> Result<(), io::Error> {
        self.current_hasher = BlockHasher::new();
        self.current_hasher.set_block(count_block);
        Ok(())
    }

    async fn reach_max_block(
        &mut self,
        bytes: &[u8],
        _count_block: usize,
    ) -> Result<[u8; 32], errors::WriteError> {
        self.current_hasher.update(bytes);
        let block_hash = self.current_hasher.clone().finalize(false);
        self.increment_verifier
            .borrow_mut()
            .verify_hash(block_hash)
            .map_err(|_| errors::WriteError::InvalidBlockHash)?;
        Ok(block_hash)
    }

    async fn post_collect(&mut self, bytes: &[u8]) -> Result<(), errors::WriteError> {
        self.current_hasher.update(bytes);
        Ok(())
    }

    async fn final_block(
        &mut self,
        count_block: usize,
    ) -> Result<Option<[u8; 32]>, errors::WriteError> {
        let block_hash = self.current_hasher.clone().finalize(false);
        self.increment_verifier
            .borrow_mut()
            .verify_hash(block_hash)
            .map_err(|_| errors::WriteError::InvalidBlockHash)?;
        Ok(Some(block_hash))
    }

    async fn finalize_tree(&mut self) -> Result<(BufCollector, [u8; 32]), errors::WriteError> {
        let mut collector = self.increment_verifier.take().finalize();
        Ok((collector, self.root_hash))
    }
}

pub type UntrustedFileWriterState = InnerWriterState<UntrustedFileWriterCollector>;

impl UntrustedFileWriterCollector {
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

    pub(crate) fn feed_proof(&mut self, proof: &[u8]) -> Result<(), errors::FeedProofError> {
        self.increment_verifier
            .borrow_mut()
            .feed_proof(proof)
            .map_err(|_| errors::FeedProofError::InvalidProof)
    }
}
