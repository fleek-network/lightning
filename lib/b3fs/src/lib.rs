#![allow(unused, dead_code)] // crate is wip.

//! This library implements a content-addressable and incrementally verifiable virtual file system
//! (a block store), to be used for [Fleek Network](https://fleek.network). It is developed around
//! the functionalities made possible efficiently by Blake3 and its tree based structure.

pub mod bucket;

pub mod proof;

/// A set of common utility functions.
pub mod utils;

/// Provides [TreeWalker](walker::TreeWalker) to iterate a tree.
pub mod walker;

pub mod collections;

pub mod directory;

pub mod verifier;

pub mod phf;
pub mod phf_play;

#[cfg(test)]
pub mod test_utils;
