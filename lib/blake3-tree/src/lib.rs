pub mod directory;
pub mod utils;

use std::borrow::Borrow;
use std::cmp::Ordering;
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

/// A buffer containing a proof for a block of data.
pub struct ProofBuf {
    // The index at which the slice starts at in the boxed buffer.
    index: usize,
    buffer: Box<[u8]>,
}

impl ProofBuf {
    fn new_internal(tree: &[[u8; 32]], walker: TreeWalker) -> Self {
        let size = walker.size_hint().0;
        let mut encoder = ProofEncoder::new(size);
        for (direction, index) in walker {
            debug_assert!(index < tree.len(), "Index overflow.");
            encoder.insert(direction, &tree[index]);
        }
        encoder.finalize()
    }

    /// Construct a new proof for the given block index from the provided
    /// tree.
    pub fn new(tree: &[[u8; 32]], block: usize) -> Self {
        Self::new_internal(tree, TreeWalker::new(block, tree.len()))
    }

    /// Construct proof for the given block number assuming that previous
    /// blocks have already been sent.
    pub fn resume(tree: &[[u8; 32]], block: usize) -> Self {
        Self::new_internal(tree, TreeWalker::resume(block, tree.len()))
    }

    /// Returns the proof as a slice.
    #[inline(always)]
    pub fn as_slice(&self) -> &[u8] {
        &self.buffer[self.index..]
    }

    /// Returns the length of the proof.
    #[inline]
    pub fn len(&self) -> usize {
        self.buffer.len() - self.index
    }

    /// Returns if the buffer is empty.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

impl AsRef<[u8]> for ProofBuf {
    #[inline(always)]
    fn as_ref(&self) -> &[u8] {
        self.as_slice()
    }
}

impl Borrow<[u8]> for ProofBuf {
    #[inline(always)]
    fn borrow(&self) -> &[u8] {
        self.as_slice()
    }
}

impl Debug for ProofBuf {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Debug::fmt(self.as_slice(), f)
    }
}

impl PartialEq<&[u8]> for ProofBuf {
    fn eq(&self, other: &&[u8]) -> bool {
        self.as_slice().eq(*other)
    }
}

pub struct ProofSizeEstimator(pub usize);

impl ProofSizeEstimator {
    pub fn new(block: usize, num_blocks: usize) -> Self {
        assert!(num_blocks > 0 && block < num_blocks);
        let tree_len = 2 * (num_blocks - 1) + 1;
        let walker = TreeWalker::new(block, tree_len);
        let count = walker.count();
        Self(count * 32 + ((count + 7) / 8))
    }
    pub fn resume(block: usize, num_blocks: usize) -> Self {
        assert!(num_blocks > 0 && block < num_blocks);
        let tree_len = 2 * (num_blocks - 1) + 1;
        let walker = TreeWalker::resume(block, tree_len);
        let count = walker.count();
        Self(count * 32 + ((count + 7) / 8))
    }
}

/// An encoder that manages a reverse buffer which can be used to convert the
/// root-to-leaf ordering of the [`TreeWalker`] to the proper stack ordering.
pub struct ProofEncoder {
    cursor: usize,
    size: usize,
    buffer: Box<[u8]>,
}

impl ProofEncoder {
    /// Create a new proof encoder for encoding a tree with the provided max number of
    /// items. An instance of ProofEncoder can not be used to encode more than the `n`
    /// items specified here. Providing an `n` smaller than the actual number of nodes
    /// can result in panics.
    pub fn new(n: usize) -> Self {
        // Compute the byte capacity for this encoder, which is 32-byte per hash and 1
        // byte per 8 one of these.
        let capacity = n * 32 + n.div_ceil(8);
        // Create a `Vec<u8>` with the given size and set its len to the byte capacity
        // it is not important for us to take care of initializing the items since the
        // type is a u8 and has no drop logic except the deallocatation of the slice
        // itself.
        let mut vec = Vec::<u8>::with_capacity(capacity);
        if capacity > 0 {
            // SAFETY: The note above explains the use case. The justification of this
            // customization over just using a regular vector is that we need to write
            // from the end of the vector to the beginning (rev push), of course we can
            // use a regular vector and just flip everything at the end, but that will
            // be more complicated.
            unsafe {
                vec.set_len(capacity);
                // Make sure the last item in the vec which is supposed to be holding the
                // non-finalized sign byte is not dirty by setting it to zero.
                *vec.get_unchecked_mut(capacity - 1) = 0;
            }
        }

        let buffer = vec.into_boxed_slice();
        debug_assert_eq!(
            buffer.len(),
            capacity,
            "The buffer is smaller than expected."
        );

        Self {
            buffer,
            cursor: capacity,
            size: 0,
        }
    }

    /// Insert a new node into the tree, the direction determines whether or not we should
    /// be flipping the stack order when we're trying to rebuild the tree later on (on the
    /// client side).
    ///
    /// # Panics
    ///
    /// If called more than the number of times specified when it got constructed.
    pub fn insert(&mut self, direction: Direction, hash: &[u8; 32]) {
        assert!(self.cursor > 0);

        // Get the current non-finalized sign byte.
        let mut sign = self.buffer[self.cursor - 1];

        self.cursor -= 32;
        self.buffer[self.cursor..self.cursor + 32].copy_from_slice(hash);

        // update the sign byte.
        if direction == Direction::Left {
            sign |= 1 << (self.size & 7);
        }

        self.size += 1;

        // Always put the sign as the leading byte of the data without moving the
        // cursor, this way the finalize can return a valid proof for when it's
        // called when the number of items does not divide 8.
        self.buffer[self.cursor - 1] = sign;

        // If we have consumed a multiple of 8 hashes so far, consume the sign byte
        // by moving the cursor.
        if self.size & 7 == 0 {
            debug_assert!(self.cursor > 0);
            self.cursor -= 1;
            // If we have more data coming in, make sure the dirty byte which will
            // be used for the next sign byte is set to zero.
            if self.cursor > 0 {
                self.buffer[self.cursor - 1] = 0;
            }
        }
    }

    /// Finalize the result of the encoder and return the proof buffer.
    pub fn finalize(mut self) -> ProofBuf {
        // Here we don't want to consume or get a mutable reference to the internal buffer
        // we have, but also we might be called when the number of passed hashes does not
        // divide 8. In this case we already have the current sign byte as the leading byte,
        // so we need to return data start one byte before the cursor.
        //
        // Furthermore we could have been returning a Vec here, but that would imply that the
        // current allocated memory would needed to be copied first into the Vec (in case the
        // cursor != 0) and then freed as well, which is not really suitable for this use case
        // we want to provide the caller with the buffer in the valid range (starting from cursor)
        // and at the same time avoid any memory copy and extra allocation and deallocation which
        // might come with dropping the box and acquiring a vec.
        //
        // This way the caller will have access to the data, and can use it the way they want,
        // for example sending it over the wire, and then once they are done with reading the
        // data they can free the used memory.
        //
        // Another idea here is to also leverage a slab allocator on the Context object which we
        // are gonna have down the line which may improve the performance (not sure how much).
        if self.size & 7 > 0 {
            debug_assert!(self.cursor > 0);
            // shit the final sign byte.
            self.buffer[self.cursor - 1] <<= 8 - (self.size & 7);
            ProofBuf {
                buffer: self.buffer,
                index: self.cursor - 1,
            }
        } else {
            ProofBuf {
                buffer: self.buffer,
                index: self.cursor,
            }
        }
    }
}

/// The logic responsible for walking a full blake3 tree from top to bottom searching
/// for a path.
pub struct TreeWalker {
    /// Index of the node we're looking for.
    target: usize,
    /// Where we're at right now.
    current: usize,
    /// Size of the current sub tree, which is the total number of
    /// leafs under the current node.
    subtree_size: usize,
}

impl TreeWalker {
    /// Construct a new [`TreeWalker`] to walk a tree of `tree_len` items (in the array
    /// representation), looking for the provided `target`-th leaf.
    pub fn new(target: usize, tree_len: usize) -> Self {
        if tree_len <= 1 {
            return Self::empty();
        }

        let walker = Self {
            // Compute the index of the n-th leaf in the array representation of the
            // tree.
            // see: https://oeis.org/A005187
            target: target * 2 - target.count_ones() as usize,
            // Start the walk from the root of the full tree, which is the last item
            // in the array representation of the tree.
            current: tree_len - 1,
            // for `k` number of leaf nodes, the total nodes of the binary tree will
            // be `n = 2k - 1`, therefore for computing the number of leaf nodes given
            // the total number of all nodes, we can use the formula `k = ceil((n + 1) / 2)`
            // and we have `ceil(a / b) = floor((a + b - 1) / b)`.
            subtree_size: (tree_len + 2) / 2,
        };

        if walker.target > walker.current {
            return Self::empty();
        }

        walker
    }

    /// Construct a new [`TreeWalker`] to walk the tree assuming that a previous walk
    /// to the previous block has been made, and does not visit the nodes that the previous
    /// walker has visited.
    ///
    /// # Panics
    ///
    /// If target is zero. It doesn't make sense to call this function with target=zero since
    /// we don't have a -1 block that is already visited.
    pub fn resume(target: usize, tree_len: usize) -> Self {
        assert_ne!(target, 0, "Block zero has no previous blocks.");

        // Compute the index of the target in the tree representation.
        let target_index = target * 2 - target.count_ones() as usize;
        // If the target is not in this tree (out of bound) or the tree size is not
        // large enough for a resume walk return the empty iterator.
        if target_index >= tree_len || tree_len < 3 {
            return Self::empty();
        }

        let distance_to_ancestor = target.trailing_zeros();
        let subtree_size = (tree_len + 2) / 2;
        let subtree_size = (1 << distance_to_ancestor).min(subtree_size - target);
        let ancestor = target_index + (subtree_size << 1) - 2;

        if subtree_size <= 1 {
            return Self::empty();
        }

        debug_assert!(distance_to_ancestor >= 1);

        Self {
            target: target_index,
            current: ancestor,
            subtree_size,
        }
    }

    #[inline(always)]
    const fn empty() -> Self {
        Self {
            target: 0,
            current: 0,
            subtree_size: 0,
        }
    }

    /// Return the index of the target element in the array representation of the
    /// complete tree.
    pub fn tree_index(&self) -> usize {
        self.target
    }
}

/// The position of a element in an element in a binary tree.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    /// The element is the current root of the tree, it's neither on the
    /// left or right side.
    Target,
    /// The element is on the left side of the tree.
    Left,
    /// The element is on the right side of the tree.
    Right,
}

impl Iterator for TreeWalker {
    type Item = (Direction, usize);

    #[inline(always)]
    fn next(&mut self) -> Option<Self::Item> {
        // If we are at a leaf node, we've already finished the traversal, and if the
        // target is greater than the current (which can only happen in the first iteration),
        // the target is already not in this tree anywhere.
        if self.subtree_size == 0 || self.target > self.current {
            return None;
        }

        if self.current == self.target {
            self.subtree_size = 0;
            return Some((Direction::Target, self.current));
        }

        // The left subtree in a blake3 tree is always guaranteed to contain a power of two
        // number of leaf (chunks), therefore the number of items on the left subtree can
        // be easily computed as the previous power of two (less than but not equal to)
        // the current items that we know our current subtree has, anything else goes
        // to the right subtree.
        let left_subtree_size = previous_pow_of_two(self.subtree_size);
        let right_subtree_size = self.subtree_size - left_subtree_size;
        // Use the formula `n = 2k - 1` to compute the total number of items on the
        // right side of this node, the index of the left node will be `n`-th item
        // before where we currently are at.
        let right_subtree_total_nodes = right_subtree_size * 2 - 1;
        let left = self.current - right_subtree_total_nodes - 1;
        let right = self.current - 1;

        match left.cmp(&self.target) {
            // The target is on the left side, so we need to prune the right subtree.
            Ordering::Equal | Ordering::Greater => {
                self.subtree_size = left_subtree_size;
                self.current = left;
                Some((Direction::Right, right))
            },
            // The target is on the right side, prune the left subtree.
            Ordering::Less => {
                self.subtree_size = right_subtree_size;
                self.current = right;
                Some((Direction::Left, left))
            },
        }
    }

    #[inline(always)]
    fn size_hint(&self) -> (usize, Option<usize>) {
        // If we're done iterating return 0.
        if self.subtree_size == 0 {
            return (0, Some(0));
        }

        // Return the upper bound as the result of the size estimation, the actual lower bound
        // can be computed more accurately but we don't really care about the accuracy of the
        // size estimate and the upper bound should be small enough for most use cases we have.
        //
        // This line is basically `ceil(log2(self.subtree_size)) + 1` which is the max depth of
        // the current subtree and one additional element + 1.
        let upper =
            usize::BITS as usize - self.subtree_size.saturating_sub(1).leading_zeros() as usize + 1;
        (upper, Some(upper))
    }
}

/// Returns the previous power of two of a given number, the returned
/// value is always less than the provided `n`.
#[inline(always)]
fn previous_pow_of_two(n: usize) -> usize {
    n.next_power_of_two() / 2
}

/// Validates that the provided number of bytes is a valid number of bytes for a proof
/// buffer. A valid proof is either
#[inline(always)]
fn is_valid_proof_len(n: usize) -> bool {
    const SEG_SIZE: usize = 32 * 8 + 1;
    let sign_bytes = n.div_ceil(SEG_SIZE);
    let hash_bytes = n - sign_bytes;
    hash_bytes & 31 == 0 && n <= 32 * 47 + 6 && ((hash_bytes / 32) >= 2 || n == 0)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Create a mock tree that has n leaf nodes, each leaf node `i` starting
    /// from 1 has their `i`-th bit set to 1, and merging two nodes is done
    /// via `|` operation.
    fn make_mock_tree(n: u8) -> Vec<u128> {
        let n = n as usize;
        assert!(n > 0 && n <= 128);
        let mut tree = Vec::with_capacity(n * 2 - 1);
        let mut stack = Vec::with_capacity(8);
        for counter in 0..n {
            let mut node = 1u128 << counter;
            let mut counter = counter;
            while counter & 1 == 1 {
                let prev = stack.pop().unwrap();
                tree.push(node);
                node |= prev;
                counter >>= 1;
            }
            stack.push(node);
            tree.push(node);
        }

        while stack.len() >= 2 {
            let a = stack.pop().unwrap();
            let b = stack.pop().unwrap();
            tree.push(a | b);
            stack.push(a | b);
        }

        tree
    }

    #[test]
    fn tree_walker() {
        for size in 2..100 {
            let tree = make_mock_tree(size);

            for start in 0..size {
                let mut walk = TreeWalker::new(start as usize, tree.len()).collect::<Vec<_>>();
                walk.reverse();

                assert_eq!(walk[0].0, Direction::Target);
                assert_eq!(tree[walk[0].1], (1 << start));

                let mut current = tree[walk[0].1];

                for (direction, i) in &walk[1..] {
                    let node = tree[*i];

                    assert_eq!(
                        node & current,
                        0,
                        "the node should not have common bits with the current node."
                    );

                    match direction {
                        Direction::Target => panic!("Target should only appear at the start."),
                        Direction::Left => {
                            assert_eq!(((current >> 1) & node).count_ones(), 1);
                            current |= node;
                        },
                        Direction::Right => {
                            assert_eq!(((current << 1) & node).count_ones(), 1);
                            current |= node;
                        },
                    }
                }

                assert_eq!(tree[tree.len() - 1], current);
            }
        }
    }

    #[test]
    fn tree_walker_one_block() {
        let walk = TreeWalker::new(0, 1).collect::<Vec<_>>();
        assert_eq!(walk.len(), 0);
    }

    #[test]
    fn tree_walker_out_of_bound() {
        let walk = TreeWalker::new(2, 3).collect::<Vec<_>>();
        assert_eq!(walk.len(), 0);
    }

    #[test]
    fn walker_partial_tree() {
        let walk = TreeWalker::resume(2, 5).collect::<Vec<_>>();
        assert_eq!(walk.len(), 0);
    }

    #[test]
    fn encoder_zero_capacity() {
        let encoder = ProofEncoder::new(0);
        assert_eq!(0, encoder.buffer.len());
        assert_eq!(encoder.finalize().len(), 0);
    }

    #[test]
    fn encoder_one_item() {
        let mut expected = Vec::<u8>::new();
        let mut hash = [0; 32];
        hash[0] = 1;
        hash[31] = 31;

        // sign byte on the left
        let mut encoder = ProofEncoder::new(1);
        encoder.insert(Direction::Left, &hash);
        expected.push(0b10000000); // sign byte
        expected.extend_from_slice(&hash);
        assert_eq!(expected.len(), encoder.buffer.len());
        assert_eq!(encoder.finalize(), expected.as_slice());

        // sign byte on the right
        let mut encoder = ProofEncoder::new(1);
        encoder.insert(Direction::Right, &hash);
        expected.clear();
        expected.push(0); // sign byte
        expected.extend_from_slice(&hash);
        assert_eq!(encoder.finalize(), expected.as_slice());
    }

    #[test]
    fn encoder_two_item() {
        let mut expected = Vec::<u8>::new();

        let mut encoder = ProofEncoder::new(2);
        encoder.insert(Direction::Right, &[0; 32]);
        encoder.insert(Direction::Right, &[1; 32]);
        expected.push(0); // sign byte
        expected.extend_from_slice(&[1; 32]);
        expected.extend_from_slice(&[0; 32]);
        assert_eq!(encoder.finalize(), expected.as_slice());

        let mut encoder = ProofEncoder::new(2);
        encoder.insert(Direction::Left, &[0; 32]);
        encoder.insert(Direction::Right, &[1; 32]);
        expected.clear();
        expected.push(0b01000000); // sign byte
        expected.extend_from_slice(&[1; 32]);
        expected.extend_from_slice(&[0; 32]);
        assert_eq!(encoder.finalize(), expected.as_slice());

        let mut encoder = ProofEncoder::new(2);
        encoder.insert(Direction::Left, &[0; 32]);
        encoder.insert(Direction::Left, &[1; 32]);
        expected.clear();
        expected.push(0b11000000); // sign byte
        expected.extend_from_slice(&[1; 32]);
        expected.extend_from_slice(&[0; 32]);
        assert_eq!(encoder.finalize(), expected.as_slice());

        let mut encoder = ProofEncoder::new(2);
        encoder.insert(Direction::Right, &[0; 32]);
        encoder.insert(Direction::Left, &[1; 32]);
        expected.clear();
        expected.push(0b10000000); // sign byte
        expected.extend_from_slice(&[1; 32]);
        expected.extend_from_slice(&[0; 32]);
        assert_eq!(expected.len(), encoder.buffer.len());
        assert_eq!(encoder.finalize(), expected.as_slice());
    }

    #[test]
    fn valid_proof_len() {
        assert!(is_valid_proof_len(0));
        assert!(!is_valid_proof_len(1));
        assert!(!is_valid_proof_len(2));
        assert!(!is_valid_proof_len(32));
        // [sign byte + 1 hash] -> not valid proof
        // since it does not expand anything.
        assert!(!is_valid_proof_len(33));
        assert!(!is_valid_proof_len(40));
        assert!(!is_valid_proof_len(64));
        assert!(is_valid_proof_len(65));

        for full_seg in 0..5 {
            let bytes = full_seg * 32 * 8 + full_seg;
            assert!(is_valid_proof_len(bytes), "failed for len={bytes}");

            for partial_seg in 1..8 {
                let bytes = bytes + 1 + partial_seg * 32;
                let is_valid = bytes > 64;
                assert_eq!(
                    is_valid_proof_len(bytes),
                    is_valid,
                    "failed for len={bytes}"
                );
                assert!(!is_valid_proof_len(bytes - 1), "failed for len={bytes}");
                assert!(!is_valid_proof_len(bytes + 1), "failed for len={bytes}");
            }
        }
    }

    #[test]
    fn proof_size_estimator() {
        let mut tree_builder = blake3::tree::HashTreeBuilder::new();
        for size in 1..100 {
            tree_builder.update(&[size as u8; 256 * 1024]);
            // clone the builder and finalize it at the current size,
            // leaving the builder for the next size
            let output = tree_builder.clone().finalize();

            // check the first block's proof buf estimation
            let mut estimator = ProofSizeEstimator::new(0, size);
            let mut proof = ProofBuf::new(&output.tree, 0);
            assert_eq!(proof.len(), estimator.0);

            // iterate over the remaining blocks and ensure the estimator outputs matches the
            // proof buf lengths
            for i in 1..size {
                proof = ProofBuf::resume(&output.tree, i);
                estimator = ProofSizeEstimator::resume(i, size);
                assert_eq!(proof.len(), estimator.0);
            }
        }
    }

    #[test]
    fn incremental_verifier_very_simple_usage() {
        let mut tree_builder = blake3::tree::HashTreeBuilder::new();
        (0..6).for_each(|i| tree_builder.update(&[i; 256 * 1024]));
        let output = tree_builder.finalize();

        let mut verifier = IncrementalVerifier::new(*output.hash.as_bytes(), 3);

        let proof = ProofBuf::new(&output.tree, 3);
        verifier.feed_proof(proof.as_slice()).unwrap();

        let mut block = blake3::tree::BlockHasher::new();
        block.set_block(3);
        block.update(&[3; 256 * 1024]);
        verifier.verify(block).unwrap();

        let proof = ProofBuf::resume(&output.tree, 4);
        verifier.feed_proof(proof.as_slice()).unwrap();

        let mut block = blake3::tree::BlockHasher::new();
        block.set_block(4);
        block.update(&[4; 256 * 1024]);
        verifier.verify(block).unwrap();
    }

    #[test]
    fn incremental_verifier_basic() {
        let mut tree_builder = blake3::tree::HashTreeBuilder::new();
        (0..4).for_each(|i| tree_builder.update(&[i; 256 * 1024]));
        let output = tree_builder.finalize();

        for i in 0..4 {
            let proof = ProofBuf::new(&output.tree, i);
            let mut verifier = IncrementalVerifier::new(*output.hash.as_bytes(), i);
            verifier.feed_proof(proof.as_slice()).unwrap();

            let mut block = blake3::tree::BlockHasher::new();
            block.set_block(i);
            block.update(&[i as u8; 256 * 1024]);
            verifier.verify(block).unwrap();

            assert_eq!(verifier.is_done(), i > 2);

            // for even blocks we should be able to also verify the next block without
            // the need to feed new proof.
            if i % 2 == 0 {
                let mut block = blake3::tree::BlockHasher::new();
                block.set_block(i + 1);
                block.update(&[i as u8 + 1; 256 * 1024]);
                verifier.verify(block).unwrap();
            }

            assert_eq!(verifier.is_done(), i > 1);
            if i > 1 {
                assert_eq!(
                    verifier.verify(blake3::tree::BlockHasher::new()),
                    Err(IncrementalVerifierError::VerifierTerminated)
                );
            }

            drop(verifier);
        }
    }

    #[test]
    fn incremental_verifier_small_data() {
        let mut tree_builder = blake3::tree::HashTreeBuilder::new();
        tree_builder.update(&[17; 64]);
        let output = tree_builder.finalize();

        let proof = ProofBuf::new(&output.tree, 0);
        assert_eq!(proof.len(), 0);

        let mut verifier = IncrementalVerifier::new(*output.hash.as_bytes(), 0);
        verifier.feed_proof(proof.as_slice()).unwrap();

        let mut block = blake3::tree::BlockHasher::new();
        block.set_block(0);
        block.update(&[17; 64]);
        verifier.verify(block).unwrap();

        assert!(verifier.is_done());

        assert_eq!(
            verifier.verify(blake3::tree::BlockHasher::new()),
            Err(IncrementalVerifierError::VerifierTerminated)
        );

        drop(verifier);
    }

    #[test]
    fn incremental_verifier_resume_simple() {
        let mut tree_builder = blake3::tree::HashTreeBuilder::new();
        (0..4).for_each(|i| tree_builder.update(&[i; 256 * 1024]));
        let output = tree_builder.finalize();

        let mut verifier = IncrementalVerifier::new(*output.hash.as_bytes(), 0);

        let proof = ProofBuf::new(&output.tree, 0);
        verifier.feed_proof(proof.as_slice()).unwrap();
        let mut block = blake3::tree::BlockHasher::new();
        block.set_block(0);
        block.update(&[0; 256 * 1024]);
        verifier.verify(block).unwrap();
        assert_eq!(verifier.block_counter, 1);
        assert_eq!(verifier.current_hash(), &output.tree[1]);

        let proof = ProofBuf::resume(&output.tree, 1);
        assert_eq!(proof.len(), 0);
        verifier.feed_proof(proof.as_slice()).unwrap();
        let mut block = blake3::tree::BlockHasher::new();
        block.set_block(1);
        block.update(&[1; 256 * 1024]);
        verifier.verify(block).unwrap();
        assert_eq!(verifier.block_counter, 2);

        // now the cursor should have moved to 5.
        //         6
        //    2        5
        // 0    1   [3  4] <- pruned
        assert_eq!(verifier.current_hash(), &output.tree[5]);
        let proof = ProofBuf::resume(&output.tree, 2);
        verifier.feed_proof(proof.as_slice()).unwrap();
        assert_eq!(verifier.current_hash(), &output.tree[3]);
        let mut block = blake3::tree::BlockHasher::new();
        block.set_block(2);
        block.update(&[2; 256 * 1024]);
        verifier.verify(block).unwrap();
        assert_eq!(verifier.block_counter, 3);
        assert_eq!(verifier.current_hash(), &output.tree[4]);

        let proof = ProofBuf::resume(&output.tree, 3);
        assert_eq!(proof.len(), 0);
        verifier.feed_proof(proof.as_slice()).unwrap();

        let mut block = blake3::tree::BlockHasher::new();
        block.set_block(3);
        block.update(&[3; 256 * 1024]);
        verifier.verify(block).unwrap();
        assert_eq!(verifier.block_counter, 4);
        assert!(verifier.is_done());

        drop(verifier);
    }

    #[test]
    fn incremental_verifier_partial_tree() {
        let mut tree_builder = blake3::tree::HashTreeBuilder::new();
        (0..3).for_each(|i| tree_builder.update(&[i; 256 * 1024]));
        let output = tree_builder.finalize();
        let mut verifier = IncrementalVerifier::new(*output.hash.as_bytes(), 0);

        let proof = ProofBuf::new(&output.tree, 0);
        verifier.feed_proof(proof.as_slice()).unwrap();
        let mut block = blake3::tree::BlockHasher::new();
        block.set_block(0);
        block.update(&[0; 256 * 1024]);
        verifier.verify(block).unwrap();
        assert_eq!(verifier.block_counter, 1);
        assert_eq!(verifier.current_hash(), &output.tree[1]);

        let proof = ProofBuf::resume(&output.tree, 1);
        assert_eq!(proof.len(), 0);
        verifier.feed_proof(proof.as_slice()).unwrap();
        let mut block = blake3::tree::BlockHasher::new();
        block.set_block(1);
        block.update(&[1; 256 * 1024]);
        verifier.verify(block).unwrap();
        assert_eq!(verifier.block_counter, 2);

        assert_eq!(verifier.current_hash(), &output.tree[3]);
        let proof = ProofBuf::resume(&output.tree, 2);
        assert_eq!(proof.len(), 0);
        verifier.feed_proof(proof.as_slice()).unwrap();
        assert_eq!(verifier.current_hash(), &output.tree[3]);
        let mut block = blake3::tree::BlockHasher::new();
        block.set_block(2);
        block.update(&[2; 256 * 1024]);
        verifier.verify(block).unwrap();
        assert_eq!(verifier.block_counter, 3);
        assert!(verifier.is_done());

        drop(verifier);
    }

    #[test]
    fn incremental_verifier_range_req() {
        let mut tree_builder = blake3::tree::HashTreeBuilder::new();
        (0..4).for_each(|i| tree_builder.update(&[i; 256 * 1024]));
        let output = tree_builder.finalize();

        let mut verifier = IncrementalVerifier::new(*output.hash.as_bytes(), 1);

        let proof = ProofBuf::new(&output.tree, 1);
        verifier.feed_proof(proof.as_slice()).unwrap();
        let mut block = blake3::tree::BlockHasher::new();
        block.set_block(1);
        block.update(&[1; 256 * 1024]);
        verifier.verify(block).unwrap();
        assert_eq!(verifier.block_counter, 2);

        assert_eq!(verifier.current_hash(), &output.tree[5]);
        let proof = ProofBuf::resume(&output.tree, 2);
        verifier.feed_proof(proof.as_slice()).unwrap();
        assert_eq!(verifier.current_hash(), &output.tree[3]);
        let mut block = blake3::tree::BlockHasher::new();
        block.set_block(2);
        block.update(&[2; 256 * 1024]);
        verifier.verify(block).unwrap();
        assert_eq!(verifier.block_counter, 3);
        assert_eq!(verifier.current_hash(), &output.tree[4]);

        let proof = ProofBuf::resume(&output.tree, 3);
        assert_eq!(proof.len(), 0);
        verifier.feed_proof(proof.as_slice()).unwrap();

        let mut block = blake3::tree::BlockHasher::new();
        block.set_block(3);
        block.update(&[3; 256 * 1024]);
        verifier.verify(block).unwrap();
        assert_eq!(verifier.block_counter, 4);
        assert!(verifier.is_done());

        drop(verifier);
    }

    #[test]
    fn incremental_verifier_large_data_first_proof_only() {
        #[inline(always)]
        fn block_data(n: usize) -> [u8; 256 * 1024] {
            let mut data = [0; 256 * 1024];
            for i in data.chunks_exact_mut(2) {
                i[0] = n as u8;
                i[1] = (n / 256) as u8;
            }
            data
        }

        const SIZE: usize = 2702;

        let mut tree_builder = blake3::tree::HashTreeBuilder::new();
        (0..SIZE).for_each(|i| tree_builder.update(&block_data(i)));
        let output = tree_builder.finalize();

        for start in (0..SIZE).step_by(127) {
            let mut verifier = IncrementalVerifier::new(*output.hash.as_bytes(), start);

            verifier
                .feed_proof(ProofBuf::new(&output.tree, start).as_slice())
                .unwrap_or_else(|_| panic!("Invalid Proof: size={SIZE} start={start}"));

            verifier
                .verify({
                    let mut block = blake3::tree::BlockHasher::new();
                    block.set_block(start);
                    block.update(&block_data(start));
                    block
                })
                .unwrap_or_else(|_| panic!("Invalid Content: size={SIZE} start={start}"));

            drop(verifier);
        }
    }

    #[test]
    fn incremental_verifier_large_data_first_one_resume() {
        #[inline(always)]
        fn block_data(n: usize) -> [u8; 256 * 1024] {
            let mut data = [0; 256 * 1024];
            for i in data.chunks_exact_mut(2) {
                i[0] = n as u8;
                i[1] = (n / 256) as u8;
            }
            data
        }

        const SIZE: usize = 654;

        let mut tree_builder = blake3::tree::HashTreeBuilder::new();
        (0..SIZE).for_each(|i| tree_builder.update(&block_data(i)));
        let output = tree_builder.finalize();

        for start in 0..SIZE - 1 {
            let mut verifier = IncrementalVerifier::new(*output.hash.as_bytes(), start);

            verifier
                .feed_proof(ProofBuf::new(&output.tree, start).as_slice())
                .unwrap_or_else(|_| panic!("Invalid Proof: size={SIZE} start={start}"));

            verifier
                .verify({
                    let mut block = blake3::tree::BlockHasher::new();
                    block.set_block(start);
                    block.update(&block_data(start));
                    block
                })
                .unwrap_or_else(|_| panic!("Invalid Content: size={SIZE} start={start}"));

            verifier
                .feed_proof(ProofBuf::resume(&output.tree, start + 1).as_slice())
                .unwrap_or_else(|_| panic!("Invalid Resume Proof: size={SIZE} start={start}"));

            verifier
                .verify({
                    let mut block = blake3::tree::BlockHasher::new();
                    block.set_block(start + 1);
                    block.update(&block_data(start + 1));
                    block
                })
                .unwrap_or_else(|_| panic!("Invalid Resume Content: size={SIZE} start={start}"));

            drop(verifier);
        }
    }

    // This is a very long running test so let's only do it sometimes.
    #[cfg(feature = "all-tests")]
    #[test]
    fn incremental_verifier_large_data() {
        #[inline(always)]
        fn block_data(n: usize) -> [u8; 256 * 1024] {
            let mut data = [0; 256 * 1024];
            for i in data.chunks_exact_mut(2) {
                i[0] = n as u8;
                i[1] = (n / 256) as u8;
            }
            data
        }

        const SIZE: usize = 2702;

        let mut tree_builder = blake3::tree::HashTreeBuilder::new();
        (0..SIZE).for_each(|i| tree_builder.update(&block_data(i)));
        let output = tree_builder.finalize();

        for start in (0..SIZE).step_by(127) {
            let mut verifier = IncrementalVerifier::new(*output.hash.as_bytes(), start);

            verifier
                .feed_proof(ProofBuf::new(&output.tree, start).as_slice())
                .unwrap_or_else(|_| panic!("Invalid Proof: size={SIZE} start={start}"));

            verifier
                .verify({
                    let mut block = blake3::tree::BlockHasher::new();
                    block.set_block(start);
                    block.update(&block_data(start));
                    block
                })
                .unwrap_or_else(|_| panic!("Invalid Content: size={SIZE} start={start}"));

            for i in start + 1..SIZE {
                verifier
                    .feed_proof(ProofBuf::resume(&output.tree, i).as_slice())
                    .unwrap_or_else(|_| {
                        panic!("Invalid Proof on Resume: size={SIZE} start={start} i={i}")
                    });

                verifier
                    .verify({
                        let mut block = blake3::tree::BlockHasher::new();
                        block.set_block(i);
                        block.update(&block_data(i));
                        block
                    })
                    .unwrap_or_else(|_| {
                        panic!("Invalid Content on Resume: size={SIZE} start={start} i={i}")
                    });
            }

            assert!(
                verifier.is_done(),
                "verifier not terminated: size={SIZE} start={start}"
            );

            drop(verifier);
        }
    }

    #[test]
    fn incremental_verifier_with_resume() {
        #[inline(always)]
        fn block_data(n: usize) -> [u8; 256 * 1024] {
            let mut data = [0; 256 * 1024];
            for i in data.chunks_exact_mut(2) {
                i[0] = n as u8;
                i[1] = (n / 256) as u8;
            }
            data
        }

        const SIZE: usize = 30;

        let mut tree_builder = blake3::tree::HashTreeBuilder::new();
        (0..SIZE).for_each(|i| tree_builder.update(&block_data(i)));
        let output = tree_builder.finalize();
        let start = 0;

        let mut verifier = IncrementalVerifier::new(*output.hash.as_bytes(), start);

        verifier
            .feed_proof(ProofBuf::new(&output.tree, start).as_slice())
            .unwrap_or_else(|_| panic!("Invalid Proof: size={SIZE} start={start}"));

        verifier
            .verify({
                let mut block = blake3::tree::BlockHasher::new();
                block.set_block(start);
                block.update(&block_data(start));
                block
            })
            .unwrap_or_else(|_| panic!("Invalid Content: size={SIZE} start={start}"));

        for i in start + 1..SIZE {
            verifier
                .feed_proof(ProofBuf::resume(&output.tree, i).as_slice())
                .unwrap_or_else(|_| {
                    panic!("Invalid Proof on Resume: size={SIZE} start={start} i={i}")
                });

            verifier
                .verify({
                    let mut block = blake3::tree::BlockHasher::new();
                    block.set_block(i);
                    block.update(&block_data(i));
                    block
                })
                .unwrap_or_else(|_| {
                    panic!("Invalid Content on Resume: size={SIZE} start={start} i={i}")
                });
        }

        assert!(
            verifier.is_done(),
            "verifier not terminated: size={SIZE} start={start}"
        );

        drop(verifier);
    }

    #[test]
    fn incremental_verifier_resume_654() {
        #[inline(always)]
        fn block_data(n: usize) -> [u8; 256 * 1024] {
            let mut data = [0; 256 * 1024];
            for i in data.chunks_exact_mut(2) {
                i[0] = n as u8;
                i[1] = (n / 256) as u8;
            }
            data
        }

        const SIZE: usize = 654;

        let mut tree_builder = blake3::tree::HashTreeBuilder::new();
        (0..SIZE).for_each(|i| tree_builder.update(&block_data(i)));
        let output = tree_builder.finalize();

        let mut verifier = IncrementalVerifier::new(*output.hash.as_bytes(), 639);

        verifier
            .feed_proof(ProofBuf::new(&output.tree, 639).as_slice())
            .unwrap_or_else(|_| panic!("Invalid Proof: size={SIZE}"));

        verifier
            .verify({
                let mut block = blake3::tree::BlockHasher::new();
                block.set_block(639);
                block.update(&block_data(639));
                block
            })
            .unwrap_or_else(|_| panic!("Invalid Content: size={SIZE}"));

        verifier
            .feed_proof(ProofBuf::resume(&output.tree, 640).as_slice())
            .unwrap_or_else(|_| panic!("Invalid Proof on Resume: size={SIZE}"));

        verifier
            .verify({
                let mut block = blake3::tree::BlockHasher::new();
                block.set_block(640);
                block.update(&block_data(640));
                block
            })
            .unwrap_or_else(|_| panic!("Invalid Content on Resume: size={SIZE}"));

        drop(verifier);
    }

    #[test]
    fn incremental_verifier_simple_keeper() {
        let mut tree_builder = blake3::tree::HashTreeBuilder::new();
        (0..6).for_each(|i| tree_builder.update(&[i; 256 * 1024]));
        let output = tree_builder.finalize();

        let mut verifier = IncrementalVerifier::new(*output.hash.as_bytes(), 0);
        verifier.preserve_tree();

        for i in 0..6 {
            let proof = if i == 0 {
                ProofBuf::new(&output.tree, 0)
            } else {
                ProofBuf::resume(&output.tree, i)
            };

            verifier.feed_proof(proof.as_slice()).unwrap();
            let mut block = blake3::tree::BlockHasher::new();
            block.set_block(i);
            block.update(&[i as u8; 256 * 1024]);
            verifier.verify(block).unwrap();
        }

        let tree = verifier.take_tree();
        assert_eq!(output.tree, tree);
    }

    #[cfg(feature = "all-tests")]
    #[test]
    fn incremental_verifier_keeper() {
        for size in 1..=255 {
            let mut tree_builder = blake3::tree::HashTreeBuilder::new();
            (0..size).for_each(|i| tree_builder.update(&[i; 256 * 1024]));
            let output = tree_builder.finalize();

            let mut verifier = IncrementalVerifier::new(*output.hash.as_bytes(), 0);
            verifier.preserve_tree();

            for i in 0..size {
                let proof = if i == 0 {
                    ProofBuf::new(&output.tree, 0)
                } else {
                    ProofBuf::resume(&output.tree, i as usize)
                };

                verifier.feed_proof(proof.as_slice()).unwrap();
                let mut block = blake3::tree::BlockHasher::new();
                block.set_block(i as usize);
                block.update(&[i; 256 * 1024]);
                verifier.verify(block).unwrap();
            }

            let tree = verifier.take_tree();
            assert_eq!(output.tree, tree);
        }
    }
}
