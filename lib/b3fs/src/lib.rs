//! This library implements a content-addressable and incrementally verifiable virtual file system
//! (a block store), to be used for [Fleek Network](https://fleek.network). It is developed around
//! the functionalities made possible efficiently by Blake3 and its tree based structure.

pub mod proof;

/// A set of common utility functions.
pub mod utils;

/// Provides [TreeWalker](walker::TreeWalker) to iterate a tree.
pub mod walker;

pub mod collections;

pub mod reader;

pub mod writer;

pub mod directory;

pub mod verifier;

#[cfg(test)]
pub mod test_utils;
