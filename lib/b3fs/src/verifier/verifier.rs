use arrayvec::ArrayVec;
use thiserror::Error;

use super::keeper::TreeKeeper;
use crate::proof::iter::ProofBufIter;
use crate::utils::is_valid_proof_len;

/// The incremental verifier that can be feed a stream of proofs and content and verifiy their
/// authenticity along the way.
pub struct IncrementalVerifier {
    iv: fleek_blake3::tree::IV,
    stack: ArrayVec<[u8; 32], 64>,
    could_be_root: bool,
    keeper: Option<TreeKeeper>,
}

#[derive(Error, Debug, PartialEq, Eq)]
pub enum IncrementalVerifierError {
    #[error("The proof provided to the verifier does not have a valid length.")]
    InvalidProofSize,
    #[error("The proof provided did not belong to the tree.")]
    HashMismatch,
    #[error("Verifier has already finished its job.")]
    VerifierTerminated,
    #[error("Invalid proof hash position.")]
    InvalidStackWalk,
}

impl IncrementalVerifier {
    fn new_internal(iv: fleek_blake3::tree::IV, root_hash: [u8; 32]) -> Self {
        Self {
            iv,
            stack: {
                let mut s = ArrayVec::new();
                s.push(root_hash);
                s
            },
            could_be_root: true,
            keeper: None,
        }
    }

    /// Create an incremental verifier that can verify normal content.
    pub fn new(hash: [u8; 32]) -> Self {
        Self::new_internal(fleek_blake3::tree::IV::new(), hash)
    }

    /// Create an incremental verifier that can verify directories.
    pub fn dir(hash: [u8; 32]) -> Self {
        Self::new_internal(crate::directory::merge::iv(), hash)
    }

    /// Enable the tree keeper part of the verifier which will collect all of the proofs together to
    /// recreate the full tree (once all of the content has been received)
    ///
    /// Once this is enabled `feed_proof` is not going to accept any proof that is not indicating a
    /// movement from counter 0 up.
    ///
    /// # Panics
    ///
    /// If called after initialization.
    pub fn enable_keeper(&mut self) {
        assert!(self.could_be_root);
        self.keeper = Some(TreeKeeper::default());
    }

    /// Return the tree that was captured.
    ///
    /// # Panics
    ///
    /// 1. If called without a prior call to `enable_keeper`.
    /// 2. If the tree has already been taken.
    /// 3. If there are still pending nodes in the content (pending verification)
    pub fn take_tree(&mut self) -> Vec<[u8; 32]> {
        assert!(self.stack.is_empty(), "The proof is not complete.");
        self.keeper.take().expect("Keeper not present.").take()
    }

    /// Feed a new proof to this verifier. The content chunk targeted for this proof must come
    /// immediately after through a call to [verify_hash](IncrementalVerifier::verify_hash).
    pub fn feed_proof(&mut self, proof: &[u8]) -> Result<(), IncrementalVerifierError> {
        if self.stack.is_empty() {
            return Err(IncrementalVerifierError::VerifierTerminated);
        }

        if !is_valid_proof_len(proof.len()) {
            return Err(IncrementalVerifierError::InvalidProofSize);
        }

        if proof.is_empty() {
            return Ok(());
        }

        let is_root = self.is_root();
        let expected_subtree_root = self.stack.pop().unwrap();
        let prev_stack_len = self.stack.len();
        let mut keeper_counter = 0;

        let mut stack = ArrayVec::<[u8; 32], 2>::new();
        for (is_on_left, hash) in ProofBufIter::new(proof) {
            if stack.is_full() {
                let right = stack.pop().unwrap();
                let left = stack.pop().unwrap();
                let parent = self.iv.merge(&left, &right, false);
                stack.push(parent);

                if let Some(keeper) = &mut self.keeper {
                    if is_on_left {
                        return Err(IncrementalVerifierError::InvalidStackWalk);
                    }
                    keeper.push(left);
                    keeper.push(right);
                    keeper.push(parent);
                    keeper_counter += 3;
                }
            }

            stack.push(*hash);
            if is_on_left && stack.is_full() {
                stack.swap(0, 1);
            }

            if !is_on_left {
                // the current node is on the right (or is target) on the walk up.
                self.stack.push(*hash);
            }
        }

        // e.g: (6 (2 (0 1)) (5 (3 4))
        // the root hash is 6 which is already in the stack.
        // to prove `0` we have sent: `0 1 5` and we pushed any non-left node to the stack:
        // `6` -> `0 1 5` -> `5 1 0`
        self.stack[prev_stack_len..].reverse();

        // stack must be full. because we returned eariler if proof.len() == 0, and
        // is_valid_proof_len should guarantee that at least 2 hashes are in the proof.
        assert_eq!(stack.len(), 2);
        let right = stack.pop().unwrap();
        let left = stack.pop().unwrap();
        let subtree_root = self.iv.merge(&left, &right, is_root);
        if let Some(keeper) = &mut self.keeper {
            keeper.push(left);
            keeper.push(right);
            keeper_counter += 2;
        }

        if subtree_root == expected_subtree_root {
            // everything is fine!
            // There can only ever be one main root at any time. For an external hasher the is_root
            // can only be set to `true` if there is only 1 node in the tree (one block of content)
            // Since we have processed a proof, this means that our tree has at least two nodes.
            // So we can see how `is_root` should always return `false` from this point forward.
            self.could_be_root = false;
            Ok(())
        } else {
            // invalid proof: we have to revert.
            while self.stack.len() > prev_stack_len {
                self.stack.pop();
            }
            // revert the changes to the keeper.
            if let Some(keeper) = &mut self.keeper {
                for _ in 0..keeper_counter {
                    keeper.pop();
                }
            }
            self.stack.push(expected_subtree_root);
            Err(IncrementalVerifierError::HashMismatch)
        }
    }

    /// Verify that the provided hash is the expected hash at this point of the progress. If `Ok`
    /// is returned the hash is accepted and the state will be moved forward. In case of an error
    /// the state of the verifier does not change and the correct hash of the same step would be
    /// required again.
    pub fn verify_hash(&mut self, hash: [u8; 32]) -> Result<(), IncrementalVerifierError> {
        if self.stack.is_empty() {
            return Err(IncrementalVerifierError::VerifierTerminated);
        }

        let expected_hash = self.stack.pop().unwrap();
        if hash == expected_hash {
            // Move the keeper forward.
            self.keeper.as_mut().map(|keeper| keeper.advance());
            Ok(())
        } else {
            // revert:
            self.stack.push(expected_hash);
            Err(IncrementalVerifierError::HashMismatch)
        }
    }

    /// Returns true if the hasher should finalize with the `is_root` flag for the next content.
    /// This should be called before [verify_hash](Self::verify_hash) and passed to `is_root`
    /// parameter that any hasher takes for correct usage.
    #[inline]
    pub fn is_root(&self) -> bool {
        self.could_be_root && self.stack.len() == 1
    }
}
