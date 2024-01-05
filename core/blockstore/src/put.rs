use async_trait::async_trait;
use blake3_tree::blake3::tree::{BlockHasher, HashTreeBuilder};
use blake3_tree::IncrementalVerifier;
use bytes::{BufMut, BytesMut};
use derive_more::IsVariant;
use lightning_interfaces::infu_collection::Collection;
use lightning_interfaces::types::{Blake3Hash, CompressionAlgorithm};
use lightning_interfaces::{
    IncrementalPutInterface,
    IndexerInterface,
    PutFeedProofError,
    PutFinalizeError,
    PutWriteError,
};
use tokio::task::JoinSet;
use tracing::error;

use crate::blockstore::BLOCK_SIZE;
use crate::config::{BLOCK_DIR, INTERNAL_DIR};
use crate::store::Store;

pub struct Putter<S, C: Collection> {
    invalidated: bool,
    buffer: BytesMut,
    mode: PutterMode,
    write_tasks: JoinSet<()>,
    store: S,
    indexer: C::IndexerInterface,
}

#[derive(IsVariant)]
enum PutterMode {
    WithIncrementalVerification {
        root_hash: [u8; 32],
        verifier: Box<IncrementalVerifier>,
    },
    Trusted {
        counter: usize,
        hasher: Box<HashTreeBuilder>,
    },
}

impl<C, S> Putter<S, C>
where
    C: Collection,
    S: Store + 'static,
{
    pub fn verifier(store: S, root: [u8; 32], indexer: C::IndexerInterface) -> Self {
        let mut verifier = IncrementalVerifier::new(root, 0);
        verifier.preserve_tree();
        Self::new(
            store,
            PutterMode::WithIncrementalVerification {
                root_hash: root,
                verifier: Box::new(verifier),
            },
            indexer,
        )
    }

    pub fn trust(store: S, indexer: C::IndexerInterface) -> Self {
        Self::new(
            store,
            PutterMode::Trusted {
                counter: 0,
                hasher: Box::new(HashTreeBuilder::new()),
            },
            indexer,
        )
    }

    fn new(store: S, mode: PutterMode, indexer: C::IndexerInterface) -> Self {
        Self {
            invalidated: false,
            buffer: BytesMut::new(),
            mode,
            write_tasks: JoinSet::new(),
            store,
            indexer,
        }
    }

    fn flush(&mut self, finalized: bool) -> Result<(), PutWriteError> {
        let block = if finalized {
            self.buffer.split() // take all reminder
        } else {
            self.buffer.split_to(BLOCK_SIZE)
        };

        let block_hash: [u8; 32];
        let block_counter;

        match &mut self.mode {
            PutterMode::WithIncrementalVerification { verifier, .. } => {
                if verifier.is_done() {
                    return Err(PutWriteError::InvalidContent);
                }

                block_counter = verifier.get_current_block_counter();

                block_hash = verifier
                    .verify({
                        let mut hasher = BlockHasher::new();
                        hasher.set_block(block_counter);
                        hasher.update(&block);
                        hasher
                    })
                    .map_err(|_| PutWriteError::InvalidContent)?;
            },
            PutterMode::Trusted { .. } if finalized => {
                unreachable!("should not reach here.");
            },
            PutterMode::Trusted { hasher, counter } => {
                block_hash = hasher.get_block_hash(*counter).unwrap();
                block_counter = *counter;
                *counter += 1;
            },
        }

        let mut store = self.store.clone();
        self.write_tasks.spawn(async move {
            let _ = store
                .insert(BLOCK_DIR, block_hash, block.as_ref(), Some(block_counter))
                .await;
        });

        Ok(())
    }
}

#[async_trait]
impl<C, S> IncrementalPutInterface for Putter<S, C>
where
    C: Collection,
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
        // For the trusted mode we do write-ahead before the flush, this way
        // when we are running the flush function the hasher has already seen
        // the future bytes of the data.
        if let PutterMode::Trusted { hasher, .. } = &mut self.mode {
            hasher.update(content);
        }

        self.buffer.put(content);

        let threshold = if self.mode.is_trusted() {
            BLOCK_SIZE
        } else {
            BLOCK_SIZE - 1
        };

        // As long as we have more data flush. always keep something for
        // the next caller.
        while self.buffer.len() > threshold {
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

        if self.mode.is_with_incremental_verification() && !self.buffer.is_empty() {
            self.flush(true).map_err(|_| PutFinalizeError::InvalidCID)?;
        }

        if let PutterMode::WithIncrementalVerification { verifier, .. } = &self.mode {
            if !verifier.is_done() {
                self.write_tasks.abort_all();
                return Err(PutFinalizeError::PartialContent);
            }
        }

        let (hash, tree) = match self.mode {
            PutterMode::WithIncrementalVerification {
                root_hash,
                mut verifier,
            } => (root_hash, verifier.take_tree()),
            PutterMode::Trusted { hasher, counter } => {
                // At finalization we should always have some bytes.
                if self.buffer.is_empty() {
                    self.write_tasks.abort_all();
                    return Err(PutFinalizeError::PartialContent);
                }

                let tmp = hasher.finalize();
                let hash = tmp.hash.into();
                let tree = tmp.tree;
                let index = counter * 2 - counter.count_ones() as usize;
                let block_hash = tree[index];
                let block = self.buffer.split();

                let mut store = self.store.clone();
                self.write_tasks.spawn(async move {
                    let _ = store
                        .insert(BLOCK_DIR, block_hash, block.as_ref(), Some(counter))
                        .await;
                });

                (hash, tree)
            },
        };

        while let Some(res) = self.write_tasks.join_next().await {
            if let Err(e) = res {
                error!("write task failed: {e:?}");
                return Err(PutFinalizeError::WriteFailed);
            }
        }

        // In future this can be a no-op/zero-copy when `flatten-slice` is stable in rust.
        let mut encoded_tree = Vec::with_capacity(32 * tree.len());
        for item in tree {
            encoded_tree.extend(&item);
        }

        self.store
            .insert(INTERNAL_DIR, hash, &encoded_tree, None)
            .await
            .map_err(|e| {
                error!("failed to write tree to store: {e:?}");
                PutFinalizeError::WriteFailed
            })?;

        self.indexer.register(hash);

        Ok(hash)
    }
}
