//! Helper wrappers around a collection of hashes.
//!
//! In this codebase we treat a list of hashes in two main ways. In one way we treat such list
//! simply as an array of hashes one appearing after another. In this way the iteration goes
//! from the first item in the array and then one after another all the way to the end. For this
//! simple method we implemented [FlatHashSlice].
//!
//! In other scenarios also common to our use cases, a series of hashes is to be treated as a
//! post-order traversal of a binary tree. The way we balance a binary tree is exactly the same
//! way as Blake3, which is simply: "All left subtrees must be a full power of 2."
//!
//! [HashTree] is implemented to be a helper for the second use case. This wrapper provides
//! iteration only over the leaf nodes of this binary tree.
//!
//! It is worth mentioning that none of the collections in this module allocate the storage
//! needed for their data on their own and are only wrapper around a slice of already stored
//! bytes.
//!
//! [FlatHashSlice]: flat::FlatHashSlice
//! [HashTree]: tree::HashTree

pub mod error;
pub mod flat;
pub mod hash_tree;
#[cfg(not(feature = "sync"))]
pub mod tree;

mod printer;

pub use flat::FlatHashSlice;
pub use hash_tree::HashTree;
