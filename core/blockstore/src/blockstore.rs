#![allow(unused)]

use std::path::PathBuf;
use std::sync::Arc;

use async_trait::async_trait;
use blake3_tree::blake3::tree::{BlockHasher, HashTreeBuilder};
use blake3_tree::IncrementalVerifier;
use bytes::{BufMut, BytesMut};
use lightning_interfaces::infu_collection::Collection;
use lightning_interfaces::types::{CompressionAlgoSet, CompressionAlgorithm};
use lightning_interfaces::{
    Blake3Hash,
    Blake3Tree,
    BlockStoreInterface,
    ConfigConsumer,
    ContentChunk,
    IncrementalPutInterface,
    PutFeedProofError,
    PutFinalizeError,
    PutWriteError,
};
use tokio::task::JoinSet;

use crate::config::Config;

const BLOCK_SIZE: usize = 256 << 10;

#[derive(Clone)]
pub struct BlockStore {}

pub struct Putter {
    invalidated: bool,
    buffer: BytesMut,
    mode: PutterMode,
    write_tasks: JoinSet<()>,
}

enum PutterMode {
    WithIncrementalVerification {
        root_hash: [u8; 32],
        verifier: IncrementalVerifier,
    },
    Trusted {
        counter: usize,
        hasher: Box<HashTreeBuilder>,
    },
}

#[async_trait]
impl<C: Collection> BlockStoreInterface<C> for BlockStore {
    type SharedPointer<T: ?Sized + Send + Sync> = Arc<T>;

    type Put = Putter;

    fn init(config: Self::Config) -> anyhow::Result<Self> {
        todo!()
    }

    async fn get_tree(&self, cid: &Blake3Hash) -> Option<Self::SharedPointer<Blake3Tree>> {
        todo!()
    }

    async fn get(
        &self,
        block_counter: u32,
        block_hash: &Blake3Hash,
        compression: CompressionAlgoSet,
    ) -> Option<Self::SharedPointer<ContentChunk>> {
        todo!()
    }

    fn put(&self, cid: Option<Blake3Hash>) -> Self::Put {
        todo!()
    }

    fn get_root_dir(&self) -> PathBuf {
        todo!()
    }
}

impl ConfigConsumer for BlockStore {
    const KEY: &'static str = "blockstore";
    type Config = Config;
}

impl Putter {
    fn flush(&mut self, finalized: bool) -> Result<(), PutWriteError> {
        let block = self.buffer.split_to(BLOCK_SIZE);
        let mut block_hash: [u8; 32];

        match &mut self.mode {
            PutterMode::WithIncrementalVerification { verifier, .. } => {
                if verifier.is_done() {
                    return Err(PutWriteError::InvalidContent);
                }

                let mut hasher = BlockHasher::new();
                hasher.set_block(verifier.get_current_block_counter());
                block_hash = hasher.finalize(verifier.is_root());

                verifier
                    .verify_hash(&block_hash)
                    .map_err(|_| PutWriteError::InvalidContent)?;
            },
            PutterMode::Trusted { hasher, counter } => {
                hasher.update(&block);

                // TODO(qti3e): we can do better than this. hasher.update
                // already does this duplicate task.
                let mut hasher = BlockHasher::new();
                hasher.set_block(*counter);
                block_hash = hasher.finalize(finalized);

                *counter += 1;
            },
        }

        let _ = block_hash;

        self.write_tasks.spawn(async {
            // write the block to file
        });

        Ok(())
    }
}

#[async_trait]
impl IncrementalPutInterface for Putter {
    fn feed_proof(&mut self, proof: &[u8]) -> Result<(), PutFeedProofError> {
        let PutterMode::WithIncrementalVerification {
            root_hash,
            verifier,
        } = &mut self.mode else {
            return Err(PutFeedProofError::UnexpectedCall);
        };

        verifier
            .feed_proof(proof)
            .map_err(|_| PutFeedProofError::InvalidProof)?;

        Ok(())
    }

    fn write(
        &mut self,
        content: &[u8],
        compression: CompressionAlgorithm,
    ) -> Result<(), PutWriteError> {
        self.buffer.put(content);

        // as long as we have more data flush. always keep something for
        // the next caller.
        while self.buffer.len() > BLOCK_SIZE {
            if let Err(e) = self.flush(false) {
                self.invalidated = true;
                self.write_tasks.abort_all();
                return Err(e);
            }
        }

        Ok(())
    }

    fn is_finished(&self) -> bool {
        // !(are we expecting more data?)
        match &self.mode {
            PutterMode::WithIncrementalVerification { verifier, .. } => {
                verifier.is_done() || self.invalidated
            },
            PutterMode::Trusted { .. } => false,
        }
    }

    async fn finalize(mut self) -> Result<Blake3Hash, PutFinalizeError> {
        if self.invalidated {
            return Err(PutFinalizeError::PartialContent);
        }

        // at finalization we should always have some bytes.
        if self.buffer.is_empty() {
            self.write_tasks.abort_all();
            return Err(PutFinalizeError::PartialContent);
        }

        self.flush(true)
            .map_err(|_| PutFinalizeError::PartialContent)?;

        if let PutterMode::WithIncrementalVerification { verifier, .. } = &self.mode {
            if !verifier.is_done() {
                self.write_tasks.abort_all();
                return Err(PutFinalizeError::PartialContent);
            }
        }

        while let Some(res) = self.write_tasks.join_next().await {
            if let Err(e) = res {
                return Err(PutFinalizeError::WriteFailed);
            }
        }

        let (_hash, _tree) = match self.mode {
            PutterMode::WithIncrementalVerification {
                root_hash,
                mut verifier,
            } => (root_hash, verifier.take_tree()),
            PutterMode::Trusted { counter, hasher } => {
                let tmp = hasher.finalize();
                (tmp.hash.into(), tmp.tree)
            },
        };

        //

        todo!()
    }
}
