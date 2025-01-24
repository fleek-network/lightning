use arrayvec::ArrayVec;
use smallvec::{Array, SmallVec};
use thiserror::Error;

use super::ProofBufIter;
use crate::hasher::iv::{SmallIV, IV};
use crate::hasher::HashTreeCollector;
use crate::utils::{is_valid_proof_len, Digest, OwnedDigest};

/// An incremental verifier can be used to verify a stream of proofs and content. It can also be
/// used to capture the entire hash tree using a collector.
pub struct IncrementalVerifier<S: VerifierCollector = ()> {
    iv: SmallIV,
    /// Number of items in the stack that are a result of a merge, which is basically
    /// the current depth of the tree.
    parent_count: u8,
    /// The static capacity here will allow holding 64 items in stack regardless of the
    /// capture mode.
    stack: SmallVec<S::Array>,
    storage: S,
}

/// An abstraction over how the [IncrementalVerifier] is supposed to collect the nodes in the hash
/// tree as it comes across them.
pub trait VerifierCollector {
    type Array: Array<Item = [u8; 32]>;
    const COLLECT: bool;

    /// Should return the updated parent counter.
    fn advance(&mut self, pc: u8, stack: &mut SmallVec<Self::Array>) -> u8;
    fn counter(&self) -> usize;
}

/// A collector that can work along-side an [`IncrementalVerifier`] to collect the entire hash tree
/// into a vector.
#[derive(Default)]
pub struct WithHashTreeCollector<T: HashTreeCollector = Vec<[u8; 32]>> {
    counter: usize,
    pub tree: T,
}

impl VerifierCollector for () {
    type Array = [[u8; 32]; 6];
    const COLLECT: bool = false;
    #[inline(always)]
    fn advance(&mut self, pc: u8, stack: &mut SmallVec<Self::Array>) -> u8 {
        stack.pop().unwrap();
        pc
    }

    #[inline(always)]
    fn counter(&self) -> usize {
        0
    }
}

impl<T: HashTreeCollector> VerifierCollector for WithHashTreeCollector<T> {
    type Array = [[u8; 32]; 12];
    const COLLECT: bool = true;

    #[inline(always)]
    fn advance(&mut self, mut pc: u8, stack: &mut SmallVec<Self::Array>) -> u8 {
        // reserve enough space for the smallest possible hash tree with the given tree depth.
        if self.counter == 0 {
            // TODO(qti3e): Figure this out.
            // self.tree.reserve(add);
        }

        // move the counter forward.
        self.counter += 1;

        // push the currently visited hash to the tree.
        self.tree.push(stack.pop().unwrap());

        let mut total_entries = self.counter;
        while (total_entries & 1) == 0 {
            total_entries >>= 1;
            let hash = stack.pop().unwrap();
            self.tree.push(hash);
            pc -= 1;
        }

        pc
    }

    #[inline(always)]
    fn counter(&self) -> usize {
        self.counter
    }
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

impl<S: VerifierCollector> IncrementalVerifier<S> {
    #[inline]
    pub fn new_with_storage(iv: SmallIV, storage: S) -> Self {
        Self {
            iv,
            stack: SmallVec::new(),
            parent_count: 0,
            storage,
        }
    }

    #[inline]
    pub fn new(iv: SmallIV) -> Self
    where
        S: Default,
    {
        Self {
            iv,
            stack: SmallVec::new(),
            parent_count: 0,
            storage: S::default(),
        }
    }

    /// Create a new incremental verifier for a directory.
    #[inline]
    pub fn dir() -> Self
    where
        S: Default,
    {
        Self::new_with_storage(SmallIV::DIR, S::default())
    }

    /// Set the root hash of the content that is to be verified. This must be called as part
    /// of the initialization of this verifier. None of the methods are meant to work before
    /// this call is made.
    ///
    /// # Panics
    ///
    /// If called more than once.
    #[inline]
    pub fn set_root_hash(&mut self, hash: [u8; 32]) {
        assert_eq!(self.parent_count, 0);
        if &hash == self.iv.empty_hash() {
            if S::COLLECT {
                // So that is_finished works and that finalize has access to the empty
                // hash in the stack.
                self.parent_count = 1;
            } else {
                return;
            }
        }
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
        let expected_hash = if S::COLLECT {
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
            if !S::COLLECT {
                self.stack.push(expected_hash);
            }
            return Err(e);
        }

        // Now we have to reverse the extension we made to both stacks.
        self.stack[old_stack_len..].reverse();

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

                if S::COLLECT {
                    self.stack.push(hash);
                    self.parent_count += 1;
                }
            }

            // Push the current item we're visiting on top of the stack. (right)
            stack.push(*hash);

            // And now check the need to move it to the left. If the flag is set.
            if should_flip {
                if S::COLLECT || i == 0 {
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
        if S::COLLECT {
            self.parent_count += 1;
        } else {
            self.parent_count = 1;
        }

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

        let expected = *self.stack.last().unwrap();
        if hash != expected {
            return Err(VerificationError::HashMismatch(
                OwnedDigest(hash),
                OwnedDigest(expected),
            ));
        }

        self.parent_count = self.storage.advance(self.parent_count, &mut self.stack);

        Ok(())
    }

    /// Returns true if the given hasher is meant to be finalized with the root flag set to true.
    pub fn is_root(&self) -> bool {
        self.stack.len() == 1
            && if S::COLLECT {
                self.storage.counter() == 0
            } else {
                self.parent_count == 0
            }
    }

    /// Returns true if the verifier has met all the content and is in a terminating state.
    pub fn is_finished(&self) -> bool {
        if S::COLLECT {
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

impl<T: HashTreeCollector> IncrementalVerifier<WithHashTreeCollector<T>> {
    /// Finalize the verifier if it is finished and returns the captured hash tree.
    ///
    /// # Panics
    ///
    /// If the hasher is not terminated.
    pub fn finalize(mut self) -> T {
        assert!(self.is_finished());
        self.storage.tree.reserve(self.stack.len());
        while let Some(hash) = self.stack.pop() {
            self.storage.tree.push(hash);
        }
        self.storage.tree
    }

    pub fn get_tree(&self) -> &T {
        &self.storage.tree
    }

    pub fn get_tree_mut(&mut self) -> &mut T {
        &mut self.storage.tree
    }

    pub fn counter(&self) -> usize {
        self.storage.counter()
    }
}

impl<S: VerifierCollector> Default for IncrementalVerifier<S>
where
    S: Default,
{
    fn default() -> Self {
        Self::new(SmallIV::DEFAULT)
    }
}

#[cfg(test)]
mod tests {
    use super::super::walker::Mode;
    use super::{IncrementalVerifier, WithHashTreeCollector};
    use crate::collections::HashTree;
    use crate::hasher::dir_hasher::hash_transcript;
    use crate::hasher::iv::SmallIV;
    use crate::test_utils::*;

    #[test]
    fn empty_checker() {
        let empty_tree = dir_hash_tree(0);
        let empty_hashtree = HashTree::try_from(&empty_tree).unwrap();

        let mut verifier = IncrementalVerifier::<()>::dir();
        verifier.set_root_hash(*empty_hashtree.root());
        assert!(verifier.is_finished());

        let mut verifier = IncrementalVerifier::<WithHashTreeCollector>::dir();
        verifier.set_root_hash(*empty_hashtree.root());
        assert!(verifier.is_finished());
        assert!(verifier.storage.tree.is_empty());
        assert_eq!(verifier.finalize(), vec![*SmallIV::DIR.empty_hash()]);
    }

    #[test]
    fn verify_from_middle() {
        for n in [3, 6, 12, 21, 27] {
            let tree = dir_hash_tree(n);
            let hashtree = HashTree::try_from(&tree).unwrap();

            let mut verifier = IncrementalVerifier::<()>::dir();
            verifier.set_root_hash(*hashtree.root());

            let mid = (n + 1) / 2;
            verifier
                .feed_proof(hashtree.generate_proof(Mode::Initial, mid).as_slice())
                .unwrap();
            verifier.verify_hash(hashtree[mid]).unwrap();
            assert!(!verifier.is_root());

            for i in mid + 1..n {
                assert!(!verifier.is_finished());

                // test against a wrong proof string.
                assert!(verifier
                    .feed_proof(hashtree.generate_proof(Mode::Initial, i).as_slice())
                    .is_err());

                verifier
                    .feed_proof(hashtree.generate_proof(Mode::Proceeding, i).as_slice())
                    .unwrap();

                // test against a wrong hash.
                assert!(verifier.verify_hash(hashtree[i - 1]).is_err());
                verifier.verify_hash(hashtree[i]).unwrap();
            }

            assert!(verifier.is_finished());
        }
    }

    #[test]
    fn demo() {
        let mut total_over_allocation = 0;
        let mut total_reallocation = 0;

        for n in 1..256 {
            let example_tree = dir_hash_tree(n);
            let example_hashtree = HashTree::try_from(&example_tree).unwrap();

            for s in [0, n / 2, n - 1] {
                let mut verifier = IncrementalVerifier::<()>::dir();
                verifier.set_root_hash(*example_hashtree.root());

                for i in s..n {
                    let proof = example_hashtree.generate_proof(Mode::from_is_initial(i == s), i);
                    verifier.feed_proof(proof.as_slice()).unwrap();
                    verifier.verify_hash(example_hashtree[i]).unwrap();
                }

                assert!(verifier.is_finished());
            }

            let mut verifier = IncrementalVerifier::<WithHashTreeCollector>::dir();
            verifier.set_root_hash(*example_hashtree.root());
            verifier.storage.tree.reserve_exact(n * 2 - 1);

            let mut reallocation = 0;

            for i in 0..n {
                let cap = verifier.storage.tree.capacity();

                let proof = example_hashtree.generate_proof(Mode::from_is_initial(i == 0), i);
                verifier.feed_proof(proof.as_slice()).unwrap();
                verifier.verify_hash(example_hashtree[i]).unwrap();

                if verifier.storage.tree.capacity() > cap {
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
    }

    #[test]
    fn is_root() {
        let tree_buffer = dir_hash_tree(1);
        let hashtree = HashTree::try_from(&tree_buffer).unwrap();
        let mut verifier = IncrementalVerifier::<WithHashTreeCollector>::dir();
        verifier.set_root_hash(*hashtree.root());
        verifier
            .feed_proof(hashtree.generate_proof(Mode::Initial, 0).as_slice())
            .unwrap();
        assert!(verifier.is_root());

        let tree_buffer = dir_hash_tree(1);
        let hashtree = HashTree::try_from(&tree_buffer).unwrap();
        let mut verifier = IncrementalVerifier::<()>::dir();
        verifier.set_root_hash(*hashtree.root());
        verifier
            .feed_proof(hashtree.generate_proof(Mode::Initial, 0).as_slice())
            .unwrap();
        assert!(verifier.is_root());

        let tree_buffer = dir_hash_tree(2);
        let hashtree = HashTree::try_from(&tree_buffer).unwrap();
        let mut verifier = IncrementalVerifier::<WithHashTreeCollector>::dir();
        verifier.set_root_hash(*hashtree.root());
        verifier
            .feed_proof(hashtree.generate_proof(Mode::Initial, 0).as_slice())
            .unwrap();
        assert!(!verifier.is_root());

        let tree_buffer = dir_hash_tree(2);
        let hashtree = HashTree::try_from(&tree_buffer).unwrap();
        let mut verifier = IncrementalVerifier::<()>::dir();
        verifier.set_root_hash(*hashtree.root());
        verifier
            .feed_proof(hashtree.generate_proof(Mode::Initial, 0).as_slice())
            .unwrap();
        assert!(!verifier.is_root());

        let tree_buffer = dir_hash_tree(2);
        let hashtree = HashTree::try_from(&tree_buffer).unwrap();
        let mut verifier = IncrementalVerifier::<()>::dir();
        verifier.set_root_hash(*hashtree.root());
        verifier
            .feed_proof(hashtree.generate_proof(Mode::Initial, 1).as_slice())
            .unwrap();
        assert!(!verifier.is_root());
    }
}
