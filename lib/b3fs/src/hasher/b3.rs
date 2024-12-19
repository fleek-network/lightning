//! These functions are copied directly from the official Blake3 repository and only made public
//! for our internal use because we need access to some of these stuff and maintaining a fork only
//! to then be imported in this crate has become an annoying act. And also given that blake3
//! algorithm is never gonna change we can simply copy paste it here.
//!
//! What we import from the upstream here is the platform specific code and [`IncrementCounter`]
//! both of which are unstable and hidden exports.
//!
//! Some of these codes in the upstream have not been modified in years which is a good sign for us
//! to be using them this way.

use core::{cmp, fmt};

use arrayref::{array_mut_ref, array_ref};
use arrayvec::{ArrayString, ArrayVec};
pub use blake3::{platform, IncrementCounter};
use platform::{Platform, MAX_SIMD_DEGREE, MAX_SIMD_DEGREE_OR_2};

use super::join;
use crate::utils::largest_power_of_two_leq;

/// The number of bytes in a [`Hash`](struct.Hash.html), 32.
pub const OUT_LEN: usize = 32;
/// The number of bytes in a key, 32.
pub const KEY_LEN: usize = 32;
pub const MAX_DEPTH: usize = 54; // 2^54 * CHUNK_LEN = 2^64
pub const BLOCK_LEN: usize = 64;
pub const CHUNK_LEN: usize = 1024;

pub const BLOCK_SIZE_IN_CHUNKS: usize = 256;
pub const MAX_BLOCK_SIZE_IN_BYTES: usize = CHUNK_LEN * BLOCK_SIZE_IN_CHUNKS;

// While iterating the compression function within a chunk, the CV is
// represented as words, to avoid doing two extra endianness conversions for
// each compression in the portable implementation. But the hash_many interface
// needs to hash both input bytes and parent nodes, so its better for its
// output CVs to be represented as bytes.
pub type CVWords = [u32; 8];
pub type CVBytes = [u8; 32]; // little-endian

pub const IV: &CVWords = &[
    0x6A09E667, 0xBB67AE85, 0x3C6EF372, 0xA54FF53A, 0x510E527F, 0x9B05688C, 0x1F83D9AB, 0x5BE0CD19,
];

// These are the internal flags that we use to domain separate root/non-root,
// chunk/parent, and chunk beginning/middle/end. These get set at the high end
// of the block flags word in the compression function, so their values start
// high and go down.
pub const CHUNK_START: u8 = 1 << 0;
pub const CHUNK_END: u8 = 1 << 1;
pub const PARENT: u8 = 1 << 2;
pub const ROOT: u8 = 1 << 3;
pub const KEYED_HASH: u8 = 1 << 4;
pub const DERIVE_KEY_CONTEXT: u8 = 1 << 5;
pub const DERIVE_KEY_MATERIAL: u8 = 1 << 6;

// Each chunk or parent node can produce either a 32-byte chaining value or, by
// setting the ROOT flag, any number of final output bytes. The Output struct
// captures the state just prior to choosing between those two possibilities.
#[derive(Clone)]
pub struct Output {
    input_chaining_value: CVWords,
    block: [u8; 64],
    block_len: u8,
    counter: u64,
    flags: u8,
    platform: Platform,
}

impl Output {
    pub fn chaining_value(&self) -> CVBytes {
        let mut cv = self.input_chaining_value;
        self.platform.compress_in_place(
            &mut cv,
            &self.block,
            self.block_len,
            self.counter,
            self.flags,
        );
        platform::le_bytes_from_words_32(&cv)
    }

    pub fn root_hash(&self) -> [u8; 32] {
        debug_assert_eq!(self.counter, 0);
        let mut cv = self.input_chaining_value;
        self.platform
            .compress_in_place(&mut cv, &self.block, self.block_len, 0, self.flags | ROOT);
        platform::le_bytes_from_words_32(&cv)
    }

    pub fn root_output_block(&self) -> [u8; 2 * OUT_LEN] {
        self.platform.compress_xof(
            &self.input_chaining_value,
            &self.block,
            self.block_len,
            self.counter,
            self.flags | ROOT,
        )
    }
}

#[derive(Clone)]
pub struct ChunkState {
    pub cv: CVWords,
    pub chunk_counter: u64,
    pub buf: [u8; BLOCK_LEN],
    pub buf_len: u8,
    pub blocks_compressed: u8,
    pub flags: u8,
    pub platform: Platform,
}

impl ChunkState {
    pub fn new(key: &CVWords, chunk_counter: u64, flags: u8, platform: Platform) -> Self {
        Self {
            cv: *key,
            chunk_counter,
            buf: [0; BLOCK_LEN],
            buf_len: 0,
            blocks_compressed: 0,
            flags,
            platform,
        }
    }

    pub fn len(&self) -> usize {
        BLOCK_LEN * self.blocks_compressed as usize + self.buf_len as usize
    }

    pub fn fill_buf(&mut self, input: &mut &[u8]) {
        let want = BLOCK_LEN - self.buf_len as usize;
        let take = cmp::min(want, input.len());
        self.buf[self.buf_len as usize..][..take].copy_from_slice(&input[..take]);
        self.buf_len += take as u8;
        *input = &input[take..];
    }

    pub fn start_flag(&self) -> u8 {
        if self.blocks_compressed == 0 {
            CHUNK_START
        } else {
            0
        }
    }

    // Try to avoid buffering as much as possible, by compressing directly from
    // the input slice when full blocks are available.
    pub fn update(&mut self, mut input: &[u8]) -> &mut Self {
        if self.buf_len > 0 {
            self.fill_buf(&mut input);
            if !input.is_empty() {
                debug_assert_eq!(self.buf_len as usize, BLOCK_LEN);
                let block_flags = self.flags | self.start_flag(); // borrowck
                self.platform.compress_in_place(
                    &mut self.cv,
                    &self.buf,
                    BLOCK_LEN as u8,
                    self.chunk_counter,
                    block_flags,
                );
                self.buf_len = 0;
                self.buf = [0; BLOCK_LEN];
                self.blocks_compressed += 1;
            }
        }

        while input.len() > BLOCK_LEN {
            debug_assert_eq!(self.buf_len, 0);
            let block_flags = self.flags | self.start_flag(); // borrowck
            self.platform.compress_in_place(
                &mut self.cv,
                array_ref!(input, 0, BLOCK_LEN),
                BLOCK_LEN as u8,
                self.chunk_counter,
                block_flags,
            );
            self.blocks_compressed += 1;
            input = &input[BLOCK_LEN..];
        }

        self.fill_buf(&mut input);
        debug_assert!(input.is_empty());
        debug_assert!(self.len() <= CHUNK_LEN);
        self
    }

    pub fn output(&self) -> Output {
        let block_flags = self.flags | self.start_flag() | CHUNK_END;
        Output {
            input_chaining_value: self.cv,
            block: self.buf,
            block_len: self.buf_len,
            counter: self.chunk_counter,
            flags: block_flags,
            platform: self.platform,
        }
    }
}

// Given some input larger than one chunk, return the number of bytes that
// should go in the left subtree. This is the largest power-of-2 number of
// chunks that leaves at least 1 byte for the right subtree.
fn left_len(content_len: usize) -> usize {
    debug_assert!(content_len > CHUNK_LEN);
    // Subtract 1 to reserve at least one byte for the right side.
    let full_chunks = (content_len - 1) / CHUNK_LEN;
    largest_power_of_two_leq(full_chunks) * CHUNK_LEN
}

// Use SIMD parallelism to hash up to MAX_SIMD_DEGREE chunks at the same time
// on a single thread. Write out the chunk chaining values and return the
// number of chunks hashed. These chunks are never the root and never empty;
// those cases use a different codepath.
pub fn compress_chunks_parallel(
    input: &[u8],
    key: &CVWords,
    chunk_counter: u64,
    flags: u8,
    platform: Platform,
    out: &mut [u8],
) -> usize {
    debug_assert!(!input.is_empty(), "empty chunks below the root");
    debug_assert!(input.len() <= MAX_SIMD_DEGREE * CHUNK_LEN);

    let mut chunks_exact = input.chunks_exact(CHUNK_LEN);
    let mut chunks_array = ArrayVec::<&[u8; CHUNK_LEN], MAX_SIMD_DEGREE>::new();
    for chunk in &mut chunks_exact {
        chunks_array.push(array_ref!(chunk, 0, CHUNK_LEN));
    }
    platform.hash_many(
        &chunks_array,
        key,
        chunk_counter,
        IncrementCounter::Yes,
        flags,
        CHUNK_START,
        CHUNK_END,
        out,
    );

    // Hash the remaining partial chunk, if there is one. Note that the empty
    // chunk (meaning the empty message) is a different codepath.
    let chunks_so_far = chunks_array.len();
    if !chunks_exact.remainder().is_empty() {
        let counter = chunk_counter + chunks_so_far as u64;
        let mut chunk_state = ChunkState::new(key, counter, flags, platform);
        chunk_state.update(chunks_exact.remainder());
        *array_mut_ref!(out, chunks_so_far * OUT_LEN, OUT_LEN) =
            chunk_state.output().chaining_value();
        chunks_so_far + 1
    } else {
        chunks_so_far
    }
}

// Use SIMD parallelism to hash up to MAX_SIMD_DEGREE parents at the same time
// on a single thread. Write out the parent chaining values and return the
// number of parents hashed. (If there's an odd input chaining value left over,
// return it as an additional output.) These parents are never the root and
// never empty; those cases use a different codepath.
pub fn compress_parents_parallel(
    child_chaining_values: &[u8],
    key: &CVWords,
    flags: u8,
    platform: Platform,
    out: &mut [u8],
) -> usize {
    debug_assert_eq!(child_chaining_values.len() % OUT_LEN, 0, "wacky hash bytes");
    let num_children = child_chaining_values.len() / OUT_LEN;
    debug_assert!(num_children >= 2, "not enough children");
    debug_assert!(num_children <= 2 * MAX_SIMD_DEGREE_OR_2, "too many");

    let mut parents_exact = child_chaining_values.chunks_exact(BLOCK_LEN);
    // Use MAX_SIMD_DEGREE_OR_2 rather than MAX_SIMD_DEGREE here, because of
    // the requirements of compress_subtree_wide().
    let mut parents_array = ArrayVec::<&[u8; BLOCK_LEN], MAX_SIMD_DEGREE_OR_2>::new();
    for parent in &mut parents_exact {
        parents_array.push(array_ref!(parent, 0, BLOCK_LEN));
    }
    platform.hash_many(
        &parents_array,
        key,
        0, // Parents always use counter 0.
        IncrementCounter::No,
        flags | PARENT,
        0, // Parents have no start flags.
        0, // Parents have no end flags.
        out,
    );

    // If there's an odd child left over, it becomes an output.
    let parents_so_far = parents_array.len();
    if !parents_exact.remainder().is_empty() {
        out[parents_so_far * OUT_LEN..][..OUT_LEN].copy_from_slice(parents_exact.remainder());
        parents_so_far + 1
    } else {
        parents_so_far
    }
}

// The wide helper function returns (writes out) an array of chaining values
// and returns the length of that array. The number of chaining values returned
// is the dynamically detected SIMD degree, at most MAX_SIMD_DEGREE. Or fewer,
// if the input is shorter than that many chunks. The reason for maintaining a
// wide array of chaining values going back up the tree, is to allow the
// implementation to hash as many parents in parallel as possible.
//
// As a special case when the SIMD degree is 1, this function will still return
// at least 2 outputs. This guarantees that this function doesn't perform the
// root compression. (If it did, it would use the wrong flags, and also we
// wouldn't be able to implement extendable output.) Note that this function is
// not used when the whole input is only 1 chunk long; that's a different
// codepath.
//
// Why not just have the caller split the input on the first update(), instead
// of implementing this special rule? Because we don't want to limit SIMD or
// multithreading parallelism for that update().
pub fn compress_subtree_wide<J: join::Join>(
    input: &[u8],
    key: &CVWords,
    chunk_counter: u64,
    flags: u8,
    platform: Platform,
    out: &mut [u8],
) -> usize {
    // Note that the single chunk case does *not* bump the SIMD degree up to 2
    // when it is 1. This allows Rayon the option of multithreading even the
    // 2-chunk case, which can help performance on smaller platforms.
    if input.len() <= platform.simd_degree() * CHUNK_LEN {
        return compress_chunks_parallel(input, key, chunk_counter, flags, platform, out);
    }

    // With more than simd_degree chunks, we need to recurse. Start by dividing
    // the input into left and right subtrees. (Note that this is only optimal
    // as long as the SIMD degree is a power of 2. If we ever get a SIMD degree
    // of 3 or something, we'll need a more complicated strategy.)
    debug_assert_eq!(platform.simd_degree().count_ones(), 1, "power of 2");
    let (left, right) = input.split_at(left_len(input.len()));
    let right_chunk_counter = chunk_counter + (left.len() / CHUNK_LEN) as u64;

    // Make space for the child outputs. Here we use MAX_SIMD_DEGREE_OR_2 to
    // account for the special case of returning 2 outputs when the SIMD degree
    // is 1.
    let mut cv_array = [0; 2 * MAX_SIMD_DEGREE_OR_2 * OUT_LEN];
    let degree = if left.len() == CHUNK_LEN {
        // The "simd_degree=1 and we're at the leaf nodes" case.
        debug_assert_eq!(platform.simd_degree(), 1);
        1
    } else {
        cmp::max(platform.simd_degree(), 2)
    };
    let (left_out, right_out) = cv_array.split_at_mut(degree * OUT_LEN);

    // Recurse! For update_rayon(), this is where we take advantage of RayonJoin and use multiple
    // threads.
    let (left_n, right_n) = J::join(
        || compress_subtree_wide::<J>(left, key, chunk_counter, flags, platform, left_out),
        || compress_subtree_wide::<J>(right, key, right_chunk_counter, flags, platform, right_out),
    );

    // The special case again. If simd_degree=1, then we'll have left_n=1 and
    // right_n=1. Rather than compressing them into a single output, return
    // them directly, to make sure we always have at least two outputs.
    debug_assert_eq!(left_n, degree);
    debug_assert!(right_n >= 1 && right_n <= left_n);
    if left_n == 1 {
        out[..2 * OUT_LEN].copy_from_slice(&cv_array[..2 * OUT_LEN]);
        return 2;
    }

    // Otherwise, do one layer of parent node compression.
    let num_children = left_n + right_n;
    compress_parents_parallel(
        &cv_array[..num_children * OUT_LEN],
        key,
        flags,
        platform,
        out,
    )
}

// Hash a subtree with compress_subtree_wide(), and then condense the resulting
// list of chaining values down to a single parent node. Don't compress that
// last parent node, however. Instead, return its message bytes (the
// concatenated chaining values of its children). This is necessary when the
// first call to update() supplies a complete subtree, because the topmost
// parent node of that subtree could end up being the root. It's also necessary
// for extended output in the general case.
//
// As with compress_subtree_wide(), this function is not used on inputs of 1
// chunk or less. That's a different codepath.
pub fn compress_subtree_to_parent_node<J: join::Join>(
    input: &[u8],
    key: &CVWords,
    chunk_counter: u64,
    flags: u8,
    platform: Platform,
) -> [u8; BLOCK_LEN] {
    debug_assert!(input.len() > CHUNK_LEN);
    let mut cv_array = [0; MAX_SIMD_DEGREE_OR_2 * OUT_LEN];
    let mut num_cvs =
        compress_subtree_wide::<J>(input, key, chunk_counter, flags, platform, &mut cv_array);
    debug_assert!(num_cvs >= 2);

    // If MAX_SIMD_DEGREE is greater than 2 and there's enough input,
    // compress_subtree_wide() returns more than 2 chaining values. Condense
    // them into 2 by forming parent nodes repeatedly.
    let mut out_array = [0; MAX_SIMD_DEGREE_OR_2 * OUT_LEN / 2];
    while num_cvs > 2 {
        let cv_slice = &cv_array[..num_cvs * OUT_LEN];
        num_cvs = compress_parents_parallel(cv_slice, key, flags, platform, &mut out_array);
        cv_array[..num_cvs * OUT_LEN].copy_from_slice(&out_array[..num_cvs * OUT_LEN]);
    }
    *array_ref!(cv_array, 0, 2 * OUT_LEN)
}

// Hash a complete input all at once. Unlike compress_subtree_wide() and
// compress_subtree_to_parent_node(), this function handles the 1 chunk case.
pub fn hash_all_at_once<J: join::Join>(input: &[u8], key: &CVWords, flags: u8) -> Output {
    let platform = Platform::detect();

    // If the whole subtree is one chunk, hash it directly with a ChunkState.
    if input.len() <= CHUNK_LEN {
        return ChunkState::new(key, 0, flags, platform)
            .update(input)
            .output();
    }

    // Otherwise construct an Output object from the parent node returned by
    // compress_subtree_to_parent_node().
    Output {
        input_chaining_value: *key,
        block: compress_subtree_to_parent_node::<J>(input, key, 0, flags, platform),
        block_len: BLOCK_LEN as u8,
        counter: 0,
        flags: flags | PARENT,
        platform,
    }
}

pub fn parent_node_output(
    left_child: &CVBytes,
    right_child: &CVBytes,
    key: &CVWords,
    flags: u8,
    platform: Platform,
) -> Output {
    let mut block = [0; BLOCK_LEN];
    block[..32].copy_from_slice(left_child);
    block[32..].copy_from_slice(right_child);
    Output {
        input_chaining_value: *key,
        block,
        block_len: BLOCK_LEN as u8,
        counter: 0,
        flags: flags | PARENT,
        platform,
    }
}
