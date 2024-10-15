use core::cmp;

use arrayref::array_ref;
use arrayvec::ArrayVec;

use super::b3::platform;
use super::iv::{SmallIV, IV};
use super::{b3, join, HashTreeCollector};
use crate::utils;

/// Block size for Fleek's proof of delivery. Which is 256KiB. This number is in Blake3
/// chunks and the code is implemented assuming this is always a power of two.
const TREE_BLOCK_SIZE_IN_CHUNK_LOG_2: usize = 8;
const TREE_BLOCK_SIZE_IN_CHUNK: usize = 1 << TREE_BLOCK_SIZE_IN_CHUNK_LOG_2;
const TREE_BLOCK_SIZE_BYTES: usize = TREE_BLOCK_SIZE_IN_CHUNK * b3::CHUNK_LEN;
const TREE_BLOCK_SIZE_IN_CHUNK_MOD_MASK: u64 = (TREE_BLOCK_SIZE_IN_CHUNK - 1) as u64;
const TREE_MAX_TREE_DEPTH: usize = b3::MAX_DEPTH - TREE_BLOCK_SIZE_IN_CHUNK_LOG_2;

/// An incremental Blake3 hasher that also maintains an array representation of the full
/// Blake3 tree, the generated array will be in the same stack ordering as Blake3.
///
/// The number is this tree are the index of each of these nodes in the array representation,
/// and because of how Blake3 functions, the left subtree will always contain a power of 2 number
/// of chunks.
///
/// ```text
///           8
///          / \
///         /   \
///        6     7
///       / \
///      /   \
///     /     \
///    2       5
///   / \     / \
///  /   \   /   \
/// 0     1 3     4
/// ```
#[derive(Clone)]
pub struct Blake3Hasher<T: HashTreeCollector = Vec<[u8; 32]>> {
    block_state: BlockHasher,
    cv_stack: ArrayVec<b3::CVBytes, { TREE_MAX_TREE_DEPTH + 1 }>,
    block_counter: u64,
    pub tree: T,
}

unsafe impl<T: HashTreeCollector> Send for Blake3Hasher<T> where T: Send {}
unsafe impl<T: HashTreeCollector> Sync for Blake3Hasher<T> where T: Sync {}

/// Incremental hasher for a single block, this can only be used to hash only one block.
#[derive(Clone)]
pub struct BlockHasher {
    iv: SmallIV,
    chunk_state: b3::ChunkState,
    cv_stack: ArrayVec<b3::CVBytes, { TREE_BLOCK_SIZE_IN_CHUNK_LOG_2 + 1 }>,
}

impl<T: HashTreeCollector> Blake3Hasher<T> {
    /// Create a new [Blake3Hasher`] with the default IV.
    pub fn new(tree: T) -> Self {
        Self::new_with_iv(tree, SmallIV::DEFAULT)
    }

    pub fn new_with_iv(tree: T, iv: SmallIV) -> Self {
        Self {
            block_state: iv.into(),
            cv_stack: Default::default(),
            block_counter: 0,
            tree,
        }
    }

    /// Input some more bytes to this hasher, this function is always single-threaded
    /// use [`Self::update_rayon`] to achieve multi-threaded implementation with rayon.
    ///
    /// Also for the best performance it is recommended that you feed full blocks to this
    /// function or at least powers of two.
    pub fn update(&mut self, input: &[u8]) {
        self.update_with_join::<join::SerialJoin>(input);
    }

    /// Just like `update` but runs the hasher using rayon for work-stealing and parallelism.
    pub fn update_rayon(&mut self, input: &[u8]) {
        self.update_with_join::<join::RayonJoin>(input)
    }

    pub fn update_with_join<J: join::Join>(&mut self, mut input: &[u8]) {
        // If we have an incomplete block, try to feed it more bytes.
        let block_state_len = self.block_state.len();
        if block_state_len > 0 {
            let want = TREE_BLOCK_SIZE_BYTES - block_state_len;
            let take = cmp::min(want, input.len());
            self.block_state.update_with_join::<J>(&input[..take]);
            input = &input[take..];

            if input.is_empty() {
                return;
            }

            // The block is filled, let's finalize it and push it to the stack.
            let block_cv = self.block_state.final_output().chaining_value();
            self.push_cv(&block_cv);
            self.block_state.move_to_next_block();

            debug_assert_eq!(
                self.block_state.chunk_state.len(),
                0,
                "chunk state must be empty"
            );
        }

        while input.len() >= 2 * TREE_BLOCK_SIZE_BYTES {
            // If the caller of this function keeps passing chunks of exact size of the fleek
            // block size, we will only ever reach this loop in the code which is super fast.
            let cv_pair = b3::compress_subtree_to_parent_node::<J>(
                &input[..2 * TREE_BLOCK_SIZE_BYTES],
                self.block_state.iv.key(),
                self.block_state.chunk_state.chunk_counter,
                self.block_state.chunk_state.flags,
                self.block_state.chunk_state.platform,
            );

            let left_cv = array_ref!(cv_pair, 0, 32);
            let right_cv = array_ref!(cv_pair, 32, 32);
            self.push_cv(left_cv);
            self.push_cv(right_cv);

            self.block_state.chunk_state.chunk_counter += 2 * TREE_BLOCK_SIZE_IN_CHUNK as u64;
            input = &input[2 * TREE_BLOCK_SIZE_BYTES..];
        }

        // If we are hashing any block other than the first one (i.e cv_stack.len() > 0), we
        // already know we don't need to use that block as a root_hash, so we can compress it
        // here.
        let is_non_first_block = !self.cv_stack.is_empty() && input.len() == TREE_BLOCK_SIZE_BYTES;
        // If we have a complete block with an additional byte, we know we can finalize it here.
        let input_larger_than_one_block = input.len() > TREE_BLOCK_SIZE_BYTES;

        if input_larger_than_one_block || is_non_first_block {
            let cv_pair = b3::compress_subtree_to_parent_node::<J>(
                &input[..TREE_BLOCK_SIZE_BYTES],
                self.block_state.iv.key(),
                self.block_state.chunk_state.chunk_counter,
                self.block_state.chunk_state.flags,
                self.block_state.chunk_state.platform,
            );

            let left_cv = array_ref!(cv_pair, 0, 32);
            let right_cv = array_ref!(cv_pair, 32, 32);
            let parent_output = b3::parent_node_output(
                left_cv,
                right_cv,
                self.block_state.iv.key(),
                self.block_state.chunk_state.flags,
                self.block_state.chunk_state.platform,
            )
            .chaining_value();

            self.push_cv(&parent_output);
            self.block_state.chunk_state.chunk_counter += TREE_BLOCK_SIZE_IN_CHUNK as u64;
            input = &input[TREE_BLOCK_SIZE_BYTES..];
        }

        debug_assert!(input.len() <= TREE_BLOCK_SIZE_BYTES);
        if !input.is_empty() {
            self.merge_cv_stack();
            self.block_state.update_with_join::<J>(input);
        }
    }

    #[inline(always)]
    fn merge_cv_stack(&mut self) {
        let post_merge_stack_len = self.block_counter.count_ones() as usize;
        while self.cv_stack.len() > post_merge_stack_len {
            let right_child = self.cv_stack.pop().unwrap();
            let left_child = self.cv_stack.pop().unwrap();
            let parent_cv = b3::parent_node_output(
                &left_child,
                &right_child,
                self.block_state.iv.key(),
                self.block_state.chunk_state.flags,
                self.block_state.chunk_state.platform,
            )
            .chaining_value();
            self.cv_stack.push(parent_cv);
            self.tree.push(parent_cv);
        }
    }

    fn push_cv(&mut self, new_cv: &b3::CVBytes) {
        self.merge_cv_stack();
        self.cv_stack.push(*new_cv);
        self.tree.push(*new_cv);
        self.block_counter += 1;
    }

    fn final_output(&mut self) -> b3::Output {
        // If the current chunk is the only chunk, that makes it the root node
        // also. Convert it directly into an Output. Otherwise, we need to
        // merge subtrees below.
        if self.cv_stack.is_empty() {
            return self.block_state.final_output();
        }

        let mut output: b3::Output;
        let mut num_cvs_remaining = self.cv_stack.len();

        if self.block_state.len() > 0 {
            debug_assert_eq!(
                self.cv_stack.len(),
                self.block_counter.count_ones() as usize,
                "cv stack does not need a merge"
            );
            output = self.block_state.final_output();
        } else {
            debug_assert!(self.cv_stack.len() >= 2);
            output = b3::parent_node_output(
                &self.cv_stack[num_cvs_remaining - 2],
                &self.cv_stack[num_cvs_remaining - 1],
                self.block_state.iv.key(),
                self.block_state.chunk_state.flags,
                self.block_state.chunk_state.platform,
            );
            num_cvs_remaining -= 2;
        }
        while num_cvs_remaining > 0 {
            self.tree.push(output.chaining_value());
            output = b3::parent_node_output(
                &self.cv_stack[num_cvs_remaining - 1],
                &output.chaining_value(),
                self.block_state.iv.key(),
                self.block_state.chunk_state.flags,
                self.block_state.chunk_state.platform,
            );
            num_cvs_remaining -= 1;
        }
        output
    }

    /// Return the result of this execution including the root hash and the tree.
    pub fn finalize(mut self) -> [u8; 32] {
        self.final_output().root_hash()
    }

    /// Like [Self::finalize]
    pub fn finalize_tree(mut self) -> (T, [u8; 32]) {
        let hash = self.final_output().root_hash();
        (self.tree, hash)
    }

    pub fn get_tree_mut(&mut self) -> &mut T {
        &mut self.tree
    }

    pub fn get_tree(&self) -> &T {
        &self.tree
    }
}

impl BlockHasher {
    /// Create a new block hasher using the default `IV`
    pub fn new() -> Self {
        From::from(&IV::DEFAULT)
    }

    pub fn update(&mut self, input: &[u8]) {
        self.update_with_join::<join::SerialJoin>(input);
    }

    pub fn update_rayon(&mut self, input: &[u8]) {
        self.update_with_join::<join::RayonJoin>(input)
    }

    pub fn update_with_join<J: join::Join>(&mut self, mut input: &[u8]) {
        debug_assert!(
            input.len() + self.len() <= TREE_BLOCK_SIZE_BYTES,
            "data for HalfBlockState exceeded its limit."
        );

        if self.chunk_state.len() > 0 {
            let want = b3::CHUNK_LEN - self.chunk_state.len();
            let take = cmp::min(want, input.len());
            self.chunk_state.update(&input[..take]);
            input = &input[take..];

            if input.is_empty() {
                return;
            }

            debug_assert_eq!(self.chunk_state.len(), b3::CHUNK_LEN);

            // There is more data coming in, which means this is not the root so we can
            // finalize the chunk state here and move the chunk counter forward.
            let chunk_cv = self.chunk_state.output().chaining_value();
            self.push_cv(&chunk_cv, self.chunk_state.chunk_counter);

            // Reset the chunk state and move the chunk_counter forward.
            self.chunk_state = b3::ChunkState::new(
                self.iv.key(),
                self.chunk_state.chunk_counter + 1,
                self.chunk_state.flags,
                self.chunk_state.platform,
            );

            // Chunk state is complete. Move on with reading the full chunks.
        }

        while input.len() > b3::CHUNK_LEN {
            debug_assert_eq!(self.chunk_state.len(), 0, "no partial chunk data");
            debug_assert_eq!(b3::CHUNK_LEN.count_ones(), 1, "power of 2 chunk len");
            let mut subtree_len = utils::largest_power_of_two_leq(input.len());
            let count_so_far = self.chunk_state.chunk_counter * b3::CHUNK_LEN as u64;
            // Shrink the subtree_len until it evenly divides the count so far.
            // We know that subtree_len itself is a power of 2, so we can use a
            // bitmasking trick instead of an actual remainder operation. (Note
            // that if the caller consistently passes power-of-2 inputs of the
            // same size, as is hopefully typical, this loop condition will
            // always fail, and subtree_len will always be the full length of
            // the input.)
            //
            // An aside: We don't have to shrink subtree_len quite this much.
            // For example, if count_so_far is 1, we could pass 2 chunks to
            // compress_subtree_to_parent_node. Since we'll get 2 CVs back,
            // we'll still get the right answer in the end, and we might get to
            // use 2-way SIMD parallelism. The problem with this optimization,
            // is that it gets us stuck always hashing 2 chunks. The total
            // number of chunks will remain odd, and we'll never graduate to
            // higher degrees of parallelism. See
            // https://github.com/BLAKE3-team/BLAKE3/issues/69.
            while (subtree_len - 1) as u64 & count_so_far != 0 {
                subtree_len /= 2;
            }
            // The shrunken subtree_len might now be 1 chunk long. If so, hash
            // that one chunk by itself. Otherwise, compress the subtree into a
            // pair of CVs.
            let subtree_chunks = (subtree_len / b3::CHUNK_LEN) as u64;
            if subtree_len <= b3::CHUNK_LEN {
                debug_assert_eq!(subtree_len, b3::CHUNK_LEN);
                self.push_cv(
                    &b3::ChunkState::new(
                        self.iv.key(),
                        self.chunk_state.chunk_counter,
                        self.chunk_state.flags,
                        self.chunk_state.platform,
                    )
                    .update(&input[..subtree_len])
                    .output()
                    .chaining_value(),
                    self.chunk_state.chunk_counter,
                );
            } else {
                // This is the high-performance happy path, though getting here
                // depends on the caller giving us a long enough input.
                let cv_pair = b3::compress_subtree_to_parent_node::<J>(
                    &input[..subtree_len],
                    self.iv.key(),
                    self.chunk_state.chunk_counter,
                    self.chunk_state.flags,
                    self.chunk_state.platform,
                );
                let left_cv = array_ref!(cv_pair, 0, 32);
                let right_cv = array_ref!(cv_pair, 32, 32);
                // Push the two CVs we received into the CV stack in order. Because
                // the stack merges lazily, this guarantees we aren't merging the
                // root.
                self.push_cv(left_cv, self.chunk_state.chunk_counter);
                self.push_cv(
                    right_cv,
                    self.chunk_state.chunk_counter + (subtree_chunks / 2),
                );
            }
            self.chunk_state.chunk_counter += subtree_chunks;
            input = &input[subtree_len..];
        }

        // What remains is 1 chunk or less. Add it to the chunk state.
        debug_assert!(input.len() <= b3::CHUNK_LEN);
        if !input.is_empty() {
            self.chunk_state.update(input);
            self.merge_cv_stack(self.chunk_state.chunk_counter);
        }
    }

    #[inline(always)]
    fn merge_cv_stack(&mut self, total_len: u64) {
        // Here we diverge from the default incremental hasher implementation,
        // the `& FLEEK_BLOCK_SIZE_IN_CHUNK_MASK` is basically `% FLEEK_BLOCK_SIZE_IN_CHUNK`,
        // and we do this because the stack of the previous blocks are not going to be part
        // of the block state.
        // Because of that at the beginning of every block, we basically reset the counter,
        // hence resetting the expected number of items already in the stack to zero as well.
        let post_merge_stack_len =
            (total_len & TREE_BLOCK_SIZE_IN_CHUNK_MOD_MASK).count_ones() as usize;
        while self.cv_stack.len() > post_merge_stack_len {
            let right_child = self.cv_stack.pop().unwrap();
            let left_child = self.cv_stack.pop().unwrap();
            let parent_output = b3::parent_node_output(
                &left_child,
                &right_child,
                self.iv.key(),
                self.chunk_state.flags,
                self.chunk_state.platform,
            );
            self.cv_stack.push(parent_output.chaining_value());
        }
    }

    fn push_cv(&mut self, new_cv: &b3::CVBytes, chunk_counter: u64) {
        self.merge_cv_stack(chunk_counter);
        self.cv_stack.push(*new_cv);
    }

    /// Make this block state ready for the next block, clears the stack and moves the
    /// chunk_counter by one.
    #[inline(always)]
    pub(crate) fn move_to_next_block(&mut self) {
        self.cv_stack.clear();
        self.chunk_state = b3::ChunkState::new(
            self.iv.key(),
            self.chunk_state.chunk_counter + 1,
            self.chunk_state.flags,
            self.chunk_state.platform,
        );

        self.chunk_state.chunk_counter -=
            self.chunk_state.chunk_counter & TREE_BLOCK_SIZE_IN_CHUNK_MOD_MASK;
    }

    /// Set the index of the block which we're hashing with this [`BlockHasher`].
    ///
    /// # Panics
    ///
    /// This function can only be called right after instantiating a [`BlockHasher`],
    /// this will panic if it gets called after any bytes were consume by the hasher.
    pub fn set_block(&mut self, block: usize) {
        assert!(
            self.chunk_state.len() == 0 && self.cv_stack.is_empty(),
            "set_block can only be called before any calls to BlockState::update()."
        );

        self.chunk_state.chunk_counter = (block as u64) << TREE_BLOCK_SIZE_IN_CHUNK_LOG_2;
    }

    /// Returns the number of bytes this block hasher has been fed so far.
    #[inline(always)]
    #[allow(clippy::len_without_is_empty)]
    pub fn len(&self) -> usize {
        let mut chunk_counter =
            (self.chunk_state.chunk_counter & TREE_BLOCK_SIZE_IN_CHUNK_MOD_MASK) as usize;

        if chunk_counter == 0 && !self.cv_stack.is_empty() {
            chunk_counter = TREE_BLOCK_SIZE_IN_CHUNK;
        }

        chunk_counter * b3::CHUNK_LEN + self.chunk_state.len()
    }

    fn final_output(&self) -> b3::Output {
        // If the current chunk is the only chunk, that makes it the root node
        // also. Convert it directly into an Output. Otherwise, we need to
        // merge subtrees below.
        if self.cv_stack.is_empty() {
            return self.chunk_state.output();
        }

        // If there are any bytes in the ChunkState, finalize that chunk and
        // merge its CV with everything in the CV stack. In that case, the work
        // we did at the end of update() above guarantees that the stack
        // doesn't contain any unmerged subtrees that need to be merged first.
        // (This is important, because if there were two chunk hashes sitting
        // on top of the stack, they would need to merge with each other, and
        // merging a new chunk hash into them would be incorrect.)
        //
        // If there are no bytes in the ChunkState, we'll merge what's already
        // in the stack. In this case it's fine if there are unmerged chunks on
        // top, because we'll merge them with each other. Note that the case of
        // the empty chunk is taken care of above.
        let mut output: b3::Output;
        let mut num_cvs_remaining = self.cv_stack.len();
        if self.chunk_state.len() > 0 {
            debug_assert_eq!(
                self.cv_stack.len(),
                (self.chunk_state.chunk_counter & TREE_BLOCK_SIZE_IN_CHUNK_MOD_MASK).count_ones()
                    as usize,
                "cv stack does not need a merge"
            );
            output = self.chunk_state.output();
        } else {
            debug_assert!(self.cv_stack.len() >= 2);
            output = b3::parent_node_output(
                &self.cv_stack[num_cvs_remaining - 2],
                &self.cv_stack[num_cvs_remaining - 1],
                self.iv.key(),
                self.chunk_state.flags,
                self.chunk_state.platform,
            );
            num_cvs_remaining -= 2;
        }
        while num_cvs_remaining > 0 {
            output = b3::parent_node_output(
                &self.cv_stack[num_cvs_remaining - 1],
                &output.chaining_value(),
                self.iv.key(),
                self.chunk_state.flags,
                self.chunk_state.platform,
            );
            num_cvs_remaining -= 1;
        }
        output
    }

    /// Returns the final output from the block hasher, if the file is only
    /// composed of one block, set `is_root` to true to get the finalized
    /// hash and not the chaining value.
    pub fn finalize(self, is_root: bool) -> [u8; 32] {
        if is_root {
            self.final_output().root_hash()
        } else {
            self.final_output().chaining_value()
        }
    }
}

impl<T: HashTreeCollector> Default for Blake3Hasher<T>
where
    T: Default,
{
    fn default() -> Self {
        Self::new(T::default())
    }
}

impl Default for BlockHasher {
    fn default() -> Self {
        Self::new()
    }
}

impl From<&'static IV> for BlockHasher {
    #[inline]
    fn from(value: &'static IV) -> Self {
        BlockHasher {
            iv: SmallIV::from_static_ref(value),
            chunk_state: b3::ChunkState::new(
                value.key(),
                0,
                value.flags(),
                platform::Platform::detect(),
            ),
            cv_stack: ArrayVec::new(),
        }
    }
}

impl From<SmallIV> for BlockHasher {
    #[inline]
    fn from(value: SmallIV) -> Self {
        let chunk_state =
            b3::ChunkState::new(value.key(), 0, value.flags(), platform::Platform::detect());
        BlockHasher {
            iv: value,
            chunk_state,
            cv_stack: ArrayVec::new(),
        }
    }
}
