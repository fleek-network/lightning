use async_trait::async_trait;
use blake3_tree::blake3::tree::{BlockHasher, HashTreeBuilder};
use blake3_tree::IncrementalVerifier;
use bytes::{BufMut, BytesMut};
use lightning_interfaces::types::CompressionAlgorithm;
use lightning_interfaces::{
    Blake3Hash,
    IncrementalPutInterface,
    PutFeedProofError,
    PutFinalizeError,
    PutWriteError,
};
use tokio::task::JoinSet;

use crate::blockstore::BLOCK_SIZE;
use crate::config::{BLOCK_DIR, INTERNAL_DIR};
use crate::store::Store;

pub struct Putter<S> {
    invalidated: bool,
    buffer: BytesMut,
    mode: PutterMode,
    write_tasks: JoinSet<()>,
    store: S,
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

impl<S> Putter<S>
where
    S: Store + 'static,
{
    pub fn verifier(store: S, root: [u8; 32]) -> Self {
        Self::new(
            store,
            PutterMode::WithIncrementalVerification {
                root_hash: root,
                verifier: IncrementalVerifier::new(root, 0),
            },
        )
    }

    pub fn trust(store: S) -> Self {
        Self::new(
            store,
            PutterMode::Trusted {
                counter: 0,
                hasher: Box::new(HashTreeBuilder::new()),
            },
        )
    }

    fn new(store: S, mode: PutterMode) -> Self {
        Self {
            invalidated: false,
            buffer: BytesMut::new(),
            mode,
            write_tasks: JoinSet::new(),
            store,
        }
    }

    fn flush(&mut self, finalized: bool) -> Result<(), PutWriteError> {
        let block = self.buffer.split_to(BLOCK_SIZE);
        let block_hash: [u8; 32];
        let block_counter;

        match &mut self.mode {
            PutterMode::WithIncrementalVerification { verifier, .. } => {
                if verifier.is_done() {
                    return Err(PutWriteError::InvalidContent);
                }

                block_counter = verifier.get_current_block_counter();

                let mut hasher = BlockHasher::new();
                hasher.set_block(block_counter);
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

                block_counter = *counter;
                *counter += 1;
            },
        }

        let mut store = self.store.clone();
        self.write_tasks.spawn(async move {
            let _ = store.insert(BLOCK_DIR, block_hash, block.to_vec(), Some(block_counter)).await;
        });

        Ok(())
    }
}

#[async_trait]
impl<S> IncrementalPutInterface for Putter<S>
where
    S: Store + 'static,
{
    fn feed_proof(&mut self, proof: &[u8]) -> Result<(), PutFeedProofError> {
        let PutterMode::WithIncrementalVerification {
            verifier,
            ..
        } = &mut self.mode else {
            return Err(PutFeedProofError::UnexpectedCall);
        };

        verifier
            .feed_proof(proof)
            .map_err(|_| PutFeedProofError::InvalidProof)?;

        Ok(())
    }

    fn write(&mut self, content: &[u8], _: CompressionAlgorithm) -> Result<(), PutWriteError> {
        self.buffer.put(content);

        // As long as we have more data flush. always keep something for
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
        // This can be interpreted as "are we expecting more data?"
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

        // At finalization we should always have some bytes.
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
                log::error!("write task failed: {e:?}");
                return Err(PutFinalizeError::WriteFailed);
            }
        }

        let (hash, tree) = match self.mode {
            PutterMode::WithIncrementalVerification {
                root_hash,
                mut verifier,
            } => (root_hash, verifier.take_tree()),
            PutterMode::Trusted { hasher, .. } => {
                let tmp = hasher.finalize();
                (tmp.hash.into(), tmp.tree)
            },
        };

        let encoded_tree = bincode::serialize(&tree).map_err(|e| {
            log::error!("failed to serialize tree: {e:?}");
            PutFinalizeError::WriteFailed
        })?;

        self.store
            .insert(INTERNAL_DIR, hash, encoded_tree, None)
            .await
            .map_err(|e| {
                log::error!("failed to write tree to store: {e:?}");
                PutFinalizeError::WriteFailed
            })?;

        Ok(hash)
    }
}
