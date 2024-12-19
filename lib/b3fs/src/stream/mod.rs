//! This module mostly deals with Merkle tree style node inclusion proofs and their verification
//! logic. Along with how we deal with represnting compact encoding of these informations.
//!
//! # What is a proof? And flip bits?
//!
//! A tree inclusion proof is simply a list of hashes which is a result of a walk on the tree
//! from our target node all the way up to the root of the tree. For example if we have the
//! follwoing tree:
//!
//! ```txt
//! (6 (2 (0 1)) (5 (3 4)))
//! ```
//!
//! To walk from the first edge (i.e 0) to the root (i.e 6) we perform the given visit `0 1 5`,
//! a verifier getting these nodes can simply perform merge operations whenever their stack has
//! 2 items in it: We push 0. Push 1. And now the stack is full. So we merge `(0 1)` which gives
//! us `2` (i.e the parent node of 0 & 1). And now we keep `2` in the stack.
//! This time we push `5` and our stack is full again. So we perform another merge: `(2 5)` which
//! will give us `6`.
//!
//! After the termination we can see that we reached and sucessfully computed the root of the tree
//! and now we can ensure that the provided contents reach to the proper root.
//!
//! But now if we wanted to prove `3` and do the same style of walk we can not simply write it as
//! `3 4 2`. Because at the last step when we get to merge `(5 2)` we end up with incorrect order
//! (it should be `(2 5)` instead). So we simply put a 'negation' sign before each item to indicate
//! that on walk up the tree this item appeared on the left-side. So our proof will instead be
//! `3 4 ~2` to convey this bit of information. And now when we push `2` and the stack is full
//! we know that we need to *flip* the stack before the merge and hence fixing this issue.
//!
//! # Encoding
//!
//! So based on the above notes we can see that we are interested in encoding an array of nodes
//! (in practices these are `[u8; 32]` hash digests), along with a sign bit. A simple implementation
//! could encode this information as a `Vec<(bool, [u8; 32])>`. But we can do a bit better by
//! grouping each of these 8 items together so that we can use an actual *single bit* for each
//! sign bit. So instead we encode it somehow similar to `Vec<(u8, Vec8<[u8; 32]>)>`.
//! If the number of items we're interested in encoding is a multiple of 8 all of the bits will
//! be used for information.
//!
//! In the codebase we call each of these `257 = 32x8 + 1` bytes a segment of the proof. The only
//! partial segment (for when we don't have a multiple of 8 items) is the first segment the can
//! be smaller than 257 bytes.
//!
//! The ordering of the sign bits corresponding to the item in the segment is from left to right.
//! The left most bit is always the sign bit of the first item in the segment, even if we are
//! dealing with a partial segment. So to encode something like `3 4 ~2` we end up with a sign bit
//! that is `0b00_10_00_00`. In this case the 5 least significant bits do not convey any information
//! of interest to us.

/// Provides [BucketStream](bucket::BucketStream) to stream either a File or a Directory exploring
/// its contents.
pub mod bucket;
/// Provides an owned buffer for writing proofs.
pub mod buffer;
/// The packed signed-prefixed encoder that can write the result of a walk on a tree into a
/// compact buffer.
pub mod encoder;
/// An iterator over a proof buffer slice extracting the packed sign bits per each iteration.
pub mod iter;
/// A pretty printer for a proof buffer slice that displays each hex value properly.
pub mod pretty;
pub mod verifier;
/// Provides [TreeWalker](walker::TreeWalker) to iterate a tree.
pub mod walker;

pub use buffer::ProofBuf;
pub use encoder::ProofEncoder;
pub use iter::{ProofBufIter, ShouldFlip};
pub use pretty::ProofBufPrettyPrinter;
