use arrayvec::ArrayVec;
use fleek_blake3::tree::IV;
use smallvec::SmallVec;
use thiserror::Error;

use crate::directory::merge::iv;
use crate::proof::ProofBufIter;
use crate::utils::{is_valid_proof_len, Digest, OwnedDigest};

pub struct IncrementalVerifier<const COLLECT: bool> {
    iv: IV,
    stack: SmallVec<[[u8; 32]; 8]>,
    parent_count: u8,
    // Used as a counter if `COLLECT` otherwise is only treated as a boolean indicating
    // `!could_be_root`.
    counter: usize,
    tree: Vec<[u8; 32]>,
}

#[derive(Debug, Error)]
pub enum VerificationError {
    #[error("The provided proof does not have a valid proof")]
    InvalidProofSize,
    #[error("The provided proof contains invalid node ordering.")]
    InvalidProofWalk,
    #[error("The provided hash does not meet the expectation")]
    HashMismatch(OwnedDigest, OwnedDigest),
    #[error("Verifier already terminated")]
    Terminated,
}

impl<const COLLECT: bool> IncrementalVerifier<COLLECT> {
    pub fn new(iv: IV) -> Self {
        Self {
            iv,
            stack: SmallVec::new(),
            parent_count: 0,
            counter: 0,
            tree: Vec::new(),
        }
    }

    /// Create a new incremental verifier for a directory.
    pub fn dir() -> Self {
        Self::new(iv())
    }

    /// Set the root hash of the content that is to be verified. This must be called as part
    /// of the initialization of this verifier. None of the methods are meant to work before
    /// this call is made.
    ///
    /// # Panics
    ///
    /// If called more than once.
    pub fn set_root_hash(&mut self, hash: [u8; 32]) {
        assert_eq!(self.parent_count, 0);
        assert_eq!(self.counter, 0);
        self.stack.push(hash);
    }

    pub fn feed_proof(&mut self, proof: &[u8]) -> Result<(), VerificationError> {
        if self.is_finished() {
            return Err(VerificationError::Terminated);
        }

        if proof.is_empty() {
            return Ok(());
        }

        if !is_valid_proof_len(proof.len()) {
            return Err(VerificationError::InvalidProofSize);
        }

        let is_root = self.is_root();
        let expected_hash = if COLLECT {
            *self.stack.last().unwrap()
        } else {
            // !COLLECT we pop, in revert we push back.
            self.stack.pop().unwrap()
        };
        let old_stack_len = self.stack.len();
        let old_parent_count = self.parent_count;

        if let Err(e) = self.feed_proof_internal(is_root, expected_hash, proof) {
            // revert:
            self.parent_count = old_parent_count;
            self.stack.truncate(old_stack_len);
            if !COLLECT {
                self.stack.push(expected_hash);
            }
            return Err(e);
        }

        // Now we have to reverse the extension we made to both stacks.
        self.stack[old_stack_len..].reverse();
        if !COLLECT {
            // if !collect counter is simply used to indicate `could_be_root`.
            self.counter = 1;
        }

        Ok(())
    }

    #[inline(always)]
    fn feed_proof_internal(
        &mut self,
        is_root: bool,
        expected_hash: [u8; 32],
        proof: &[u8],
    ) -> Result<(), VerificationError> {
        let mut stack = ArrayVec::<[u8; 32], 2>::new();
        for (i, (should_flip, hash)) in ProofBufIter::new(proof).enumerate() {
            if stack.is_full() {
                let right = stack.pop().unwrap();
                let left = stack.pop().unwrap();
                let hash = self.iv.merge(&left, &right, false);
                stack.push(hash);

                if COLLECT {
                    self.stack.push(hash);
                    self.parent_count += 1;
                }
            }

            // Push the current item we're visiting on top of the stack. (right)
            stack.push(*hash);

            // And now check the need to move it to the left. If the flag is set.
            if should_flip {
                if COLLECT || i == 0 {
                    // If we are collecting, we expect a tree iteration from node id=0 which would
                    // result in no need to flip ever.
                    return Err(VerificationError::InvalidProofWalk);
                }

                stack.swap(0, 1);
            } else {
                self.stack.push(*hash);
            }
        }

        // Because of `is_valid_proof_len` we know proof had at least 2 hashes inside.
        debug_assert!(stack.is_full());

        let right = stack.pop().unwrap();
        let left = stack.pop().unwrap();
        let hash = self.iv.merge(&left, &right, is_root);
        self.parent_count += 1;

        if hash != expected_hash {
            return Err(VerificationError::HashMismatch(
                OwnedDigest(hash),
                OwnedDigest(expected_hash),
            ));
        }

        Ok(())
    }

    /// Verify the provided hash.
    pub fn verify_hash(&mut self, hash: [u8; 32]) -> Result<(), VerificationError> {
        if self.is_finished() {
            return Err(VerificationError::Terminated);
        }

        let expected = self.stack.pop().unwrap();
        if hash != expected {
            self.stack.push(expected);
            return Err(VerificationError::HashMismatch(
                OwnedDigest(hash),
                OwnedDigest(expected),
            ));
        }

        if COLLECT {
            // reserve enough space for the smallest possible hash tree with the given tree depth.
            if self.tree.capacity() == 0 {
                let add = 1 << (self.parent_count.saturating_sub(1) as usize) + 1;
                self.tree.reserve_exact(add);
            }

            // move the counter forward.
            self.counter += 1;

            // push the currently visited hash to the tree.
            self.tree.push(expected);

            let mut total_entries = self.counter;
            while (total_entries & 1) == 0 {
                total_entries >>= 1;
                let hash = self.stack.pop().unwrap();
                self.tree.push(hash);
                self.parent_count -= 1;
            }
        }

        Ok(())
    }

    /// Returns true if the given hasher is meant to be finalized with the root flag set to true.
    pub fn is_root(&self) -> bool {
        self.stack.len() == 1 && self.counter == 0
    }

    /// Returns true if the verifier has met all the content and is in a terminating state.
    ///
    /// # Limitation
    ///
    /// Currently, this method will return `false` even if the root hash is the hash of an empty
    /// content under the provided *IV*.
    pub fn is_finished(&self) -> bool {
        // TODO(qti3e): Handle hash of an empty content.
        if COLLECT {
            self.stack.len() == self.parent_count as usize
        } else {
            self.stack.is_empty()
        }
    }

    #[allow(unused)]
    fn debug_stack(&self) -> Vec<Digest> {
        self.stack.iter().map(Digest).collect()
    }
}

impl IncrementalVerifier<true> {
    /// Finalize the verifier if it is finished and returns the captured hash tree.
    ///
    /// # Panics
    ///
    /// If the hasher is not terminated.
    pub fn finalize(mut self) -> Vec<[u8; 32]> {
        assert!(self.is_finished());
        self.tree.reserve_exact(self.stack.len());
        while let Some(hash) = self.stack.pop() {
            self.tree.push(hash);
        }
        self.tree
    }

    /// Returns the remaining capacity of the buffer that is being used to capture the tree.
    pub fn remaining_capacity(&self) -> usize {
        self.tree.capacity() - self.tree.len()
    }

    /// Returns the captured hash tree up to this point. This can be used if the captured hash tree
    /// can incrementally be moved to another place. (e.g., be written to the file system)
    pub fn flush(&mut self) -> Vec<[u8; 32]> {
        let mut tree = Vec::with_capacity(self.tree.capacity() - self.tree.len());
        std::mem::swap(&mut self.tree, &mut tree);
        tree
    }

    /// Reserve exactly enough space for additional given hashes. Should be preferred if no future
    /// allocation is going to be requested.
    pub fn reserve_exact(&mut self, additional: usize) {
        self.tree.reserve_exact(additional * 2 - 1);
    }

    /// Reserves enough space to capture the tree of additional new nodes. This may allocate more
    /// space if necessary to avoid the future need for further allocations.
    pub fn reserve(&mut self, additional: usize) {
        self.tree.reserve(additional * 2 - 1);
    }
}

impl<const COLLECT: bool> Default for IncrementalVerifier<COLLECT> {
    fn default() -> Self {
        Self::new(IV::new())
    }
}

#[cfg(test)]
mod tests {
    use crate::collections::HashTree;
    use crate::test_utils::*;
    use crate::verifier::IncrementalVerifier;
    use crate::walker::Mode;

    #[test]
    fn demo() {
        let mut total_over_allocation = 0;
        let mut total_reallocation = 0;

        for n in 1..256 {
            let example_tree = dir_hash_tree(n);
            let example_hashtree = HashTree::try_from(&example_tree).unwrap();

            for s in [0, n / 2, n - 1] {
                let mut verifier = IncrementalVerifier::<false>::dir();
                verifier.set_root_hash(*example_hashtree.root());

                for i in s..n {
                    let proof = example_hashtree.generate_proof(Mode::from_is_initial(i == s), i);
                    verifier.feed_proof(proof.as_slice()).unwrap();
                    verifier.verify_hash(example_hashtree[i]).unwrap();
                }

                assert!(verifier.is_finished());
            }

            let mut verifier = IncrementalVerifier::<true>::dir();
            verifier.set_root_hash(*example_hashtree.root());
            verifier.reserve_exact(n);

            let mut reallocation = 0;

            for i in 0..n {
                let cap = verifier.tree.capacity();

                let proof = example_hashtree.generate_proof(Mode::from_is_initial(i == 0), i);
                verifier.feed_proof(proof.as_slice()).unwrap();
                verifier.verify_hash(example_hashtree[i]).unwrap();

                if verifier.tree.capacity() > cap {
                    reallocation += 1;
                }
            }
            assert!(verifier.is_finished());

            // check the captured tree and over allocations.
            let actual_tree = verifier.finalize();
            assert_eq!(example_tree, actual_tree);

            let over_allocation = actual_tree.capacity() - actual_tree.len();
            total_over_allocation += over_allocation;
            total_reallocation += reallocation;
            // println!("n={n}: over_allocation={}\tgrow={}", over_allocation, grow);
        }

        // since reserve_exact is used:
        assert_eq!(total_reallocation, 0);
        assert_eq!(total_over_allocation, 0);

        // dbg!(total_over_allocation);
        // dbg!(total_over_allocation / 255);
        // dbg!(total_reallocation);
        // println!("{}", std::mem::size_of::<IncrementalVerifier<false>>());
    }

    // #[test]
    // fn x() {
    //     for n in 1..256 {
    //         let example_tree = dir_hash_tree(n);
    //         let example_hashtree = HashTree::try_from(&example_tree).unwrap();

    //         let mut verifier = IncrementalVerifier::<true>::dir();
    //         verifier.set_root_hash(*example_hashtree.root());

    //         let proof = example_hashtree.generate_proof(Mode::Initial, 0);
    //         verifier.feed_proof(proof.as_slice()).unwrap();
    //         println!(
    //             "n={n} -> pc={} -> s={}",
    //             verifier.parent_count,
    //             example_hashtree.len()
    //         );
    //     }
    // }
}
