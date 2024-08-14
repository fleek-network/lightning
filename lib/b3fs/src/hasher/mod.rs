//! Implementation of high performance blake3 hashers that keep track of the intermediary trees.
//!
//! This provides hashing ability both for content and directory.

pub(crate) mod b3;
pub(crate) mod join;

pub mod byte_hasher;
pub mod dir_hasher;

pub trait HashTreeCapture {}
