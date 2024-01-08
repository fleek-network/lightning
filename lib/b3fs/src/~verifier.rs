use std::collections::VecDeque;
use std::fmt::Debug;

use arrayref::array_ref;
use arrayvec::ArrayVec;
pub use fleek_blake3 as blake3;
use thiserror::Error;

/// An incremental verifier that can consume a stream of proofs and content
/// and verify the integrity of the content using a blake3 root hash.
pub struct IncrementalVerifier {
    /// The configuration the Blake3.
    iv: blake3::tree::IV,
    /// The root hash of the content we're verifying.
    root_hash: [u8; 32],
    /// An optional tree keeper which we feed information to that will construct
    /// and keep the full internal blake3 tree. This should be able to construct
    /// the tree that [HashTreeBuilder](blake3::tree::HashTreeBuilder) produces,
    /// only usable when we start the incremental verifier at block 0.
    keeper: Option<TreeKeeper>,
    /// The index of the block we're verifying now, starting from zero.
    block_counter: usize,
    /// Leaf nodes of the current tree from right to left. The right most
    /// leaf node has the index 0 and nodes closer to the left have a higher
    /// index in this vec.
    nodes: ArrayVec<[u8; 32], 47>,
}

#[derive(Error, Debug, PartialEq, Eq)]
pub enum IncrementalVerifierError {
    #[error("The proof provided to the verifier does not have a valid length.")]
    InvalidProofSize,
    #[error("The proof provided did not belong to the tree.")]
    HashMismatch,
    #[error("Verifier has already finished its job.")]
    VerifierTerminated,
    #[error("Invalid counter value")]
    InvalidCounter,
    #[error("Invalid range provided for requested key.")]
    InvalidRange,
}

#[derive(Default)]
struct TreeKeeper {
    block_counter: usize,
    queue: VecDeque<[u8; 32]>,
    tree: Vec<[u8; 32]>,
}

impl IncrementalVerifier {
    /// Create a new incremental verifier that verifies an stream of proofs and
    /// content against the provided root hash.
    ///
    /// The `starting_block` determines where the content stream will start from.
    pub fn new(root_hash: [u8; 32], starting_block: usize) -> Self {
        let mut nodes = ArrayVec::new_const();
        nodes.push(root_hash);

        Self {
            iv: blake3::tree::IV::new(),
            root_hash,
            keeper: None,
            block_counter: starting_block,
            nodes,
        }
    }

    /// Enable preserving the full tree.
    ///
    /// # Panics
    ///
    /// 1. If we are already past initialization.
    /// 2. If we haven't started verifier at `starting_block = 0`.
    pub fn preserve_tree(&mut self) {
        assert!(
            (self.block_counter == 0) && self.nodes.len() == 1,
            "preserve_tree can only work when starting at block 0"
        );

        self.keeper = Some(TreeKeeper::default());
    }

    /// Set a custom initialization vector for the hasher. This is used for subtree
    /// merge operations.
    pub fn set_iv(&mut self, iv: blake3::tree::IV) {
        self.iv = iv;
    }

    #[inline]
    pub fn get_current_block_counter(&self) -> usize {
        self.block_counter
    }

    /// Returns the tree that we kept.
    ///
    /// # Panics
    ///
    /// 1. If called before finalization.
    /// 2. If called more than once.
    /// 3. If we haven't made a call to `preserve_tree` on init.
    pub fn take_tree(&mut self) -> Vec<[u8; 32]> {
        assert!(self.is_done());
        self.keeper
            .take()
            .expect("A prior call to `preserve_tree` was expected.")
            .finalize()
    }

    /// Returns true if the stream is complete.
    pub fn is_done(&self) -> bool {
        self.nodes.is_empty()
    }

    /// Verify the new block of data only by providing its hash, you should be aware of
    /// what mode you have finalized the block at.
    ///
    /// To decide if the `is_root` needs to be set you should make a call to [`Self::is_root`].
    pub fn verify_hash(&mut self, hash: &[u8; 32]) -> Result<(), IncrementalVerifierError> {
        if self.is_done() {
            return Err(IncrementalVerifierError::VerifierTerminated);
        }

        // 1. Hash the content using a block hasher with the current index.
        // 2. Compare to the hash we have under the cursor.
        // 3. Move to the next node.

        if hash != self.current_hash() {
            return Err(IncrementalVerifierError::HashMismatch);
        }

        // Move to next.
        self.nodes.pop();
        self.block_counter += 1;
        if let Some(keeper) = &mut self.keeper {
            keeper.push(*hash);
        }

        Ok(())
    }

    /// Verify the new block.
    pub fn verify(
        &mut self,
        block: blake3::tree::BlockHasher,
    ) -> Result<[u8; 32], IncrementalVerifierError> {
        if self.is_done() {
            return Err(IncrementalVerifierError::VerifierTerminated);
        }

        let hash = block.finalize(self.is_root());
        self.verify_hash(&hash).map(|_| hash)
    }

    /// Feed some new proof to the verifier which it can use to expand its internal
    /// blake3 tree.
    pub fn feed_proof(&mut self, mut proof: &[u8]) -> Result<(), IncrementalVerifierError> {
        const SEGMENT_SIZE: usize = 32 * 8 + 1;

        if self.is_done() {
            return Err(IncrementalVerifierError::VerifierTerminated);
        }

        if !is_valid_proof_len(proof.len()) {
            return Err(IncrementalVerifierError::InvalidProofSize);
        }

        if proof.is_empty() {
            return Ok(());
        }

        let mut stack: ArrayVec<[u8; 32], 2> = ArrayVec::new_const();
        let mut visited: ArrayVec<&[u8; 32], 47> = ArrayVec::new_const();
        let mut merged: ArrayVec<[u8; 32], 47> = ArrayVec::new_const();
        let keep = self.keeper.is_some();

        // Number of bytes to read per iteration. For the first iteration read
        // the partial first segment and we will then start reading full segments.
        let mut read = proof.len() % SEGMENT_SIZE;
        // Handle the case where we have complete segments.
        if read == 0 {
            read = SEGMENT_SIZE;
        }

        while !proof.is_empty() {
            // The `is_valid_proof_len` should not allow this to happen.
            debug_assert!((read - 1) % 32 == 0);
            debug_assert!(proof.len() >= read);

            let sign = proof[0];

            for (i, hash) in proof[1..read].chunks_exact(32).enumerate() {
                let hash = array_ref![hash, 0, 32];
                let should_flip = (1 << (8 - i - 1)) & sign != 0;

                // Merge the two items in the stack if it's full.
                if stack.is_full() {
                    let right_cv = stack.pop().unwrap();
                    let left_cv = stack.pop().unwrap();
                    let parent_cv = self.iv.merge(&left_cv, &right_cv, false);
                    stack.push(parent_cv);
                    if keep {
                        merged.push(parent_cv);
                    }
                }

                stack.push(*hash);

                if should_flip && stack.is_full() {
                    stack.swap(0, 1);
                }

                // If a hash is not to the left of a deeper level of the tree insert it to the
                // `visited` stack. At the end this will give us the list of all the new leaf
                // nodes from left to right which then we can insert in reverse order to
                // `self.nodes`
                if !should_flip {
                    visited.push(hash);
                }
            }

            // Move to the next part of the proof.
            proof = &proof[read..];

            // After the first partial segment revert to reading full segments (8 hash at a time).
            read = SEGMENT_SIZE;
        }

        // Now we need to finalize the stack, here it becomes important if we are merging the root
        // or not.
        assert!(stack.is_full());

        let is_root = self.is_root();
        let expected = self.nodes.pop().unwrap();

        let right_cv = stack.pop().unwrap();
        let left_cv = stack.pop().unwrap();
        let actual = self.iv.merge(&left_cv, &right_cv, is_root);

        if keep {
            merged.push(actual);
        }

        if expected != actual {
            // revert the state on self.
            self.nodes.push(expected);
            return Err(IncrementalVerifierError::HashMismatch);
        }

        for item in visited.into_iter().rev() {
            self.nodes.push(*item);
        }

        if let Some(keeper) = &mut self.keeper {
            for item in merged.into_iter().rev() {
                keeper.push_internal_rev(item);
            }
        }

        Ok(())
    }

    /// Returns true if the current cursor is pointing to the root of the tree.
    #[inline(always)]
    pub fn is_root(&self) -> bool {
        self.nodes.len() == 1 && self.nodes.last().unwrap() == &self.root_hash
    }

    /// Returns the hash of the current node in the tree.
    #[inline(always)]
    fn current_hash(&self) -> &[u8; 32] {
        self.nodes.last().unwrap()
    }
}

impl TreeKeeper {
    #[inline(always)]
    pub fn push_internal_rev(&mut self, hash: [u8; 32]) {
        self.queue.push_front(hash);
    }

    #[inline(always)]
    pub fn push(&mut self, hash: [u8; 32]) {
        self.tree.push(hash);
        let mut counter = self.block_counter;
        self.block_counter += 1;

        while (counter & 1) == 1 {
            let hash = self.queue.pop_front().unwrap();
            self.tree.push(hash);
            counter >>= 1;
        }
    }

    #[inline(always)]
    pub fn finalize(mut self) -> Vec<[u8; 32]> {
        while !self.queue.is_empty() {
            let hash = self.queue.pop_front().unwrap();
            self.tree.push(hash);
        }
        self.tree
    }
}
