#![allow(unused, dead_code)] // crate is wip.

//! This library implements a content-addressable and incrementally verifiable virtual file system
//! (a block store), to be used for [Fleek Network](https://fleek.network). It is developed around
//! the functionalities made possible efficiently by Blake3 and its tree based structure.

// Some numbers to know:
//
// 1. A hash tree of `n` items has `2n + 1` hashes.
// 2. Each hash is 32 bytes:
// 3. A directory has at most 65535 items. (Fits in u16)
// 4. Resulting hashtree of the largest directory has 3.9MB of data.
// 5. Each chunk of a file is 256KB.
// 6. For a `nMB` file the size of the hashtree is: `256n - 32` bytes.
//
// Tree    | Dir Entries   | Max File Size
// 96B     | 2             | 512KB
// 128B    | 2             | 512KB
// 256B    | 4             | 1MB
// 512B    | 8             | 2MB
// 1KB     | 16            | 4MB
// 4KB     | 64            | 16MB
// 8KB     | 128           | 32MB
// 16KB    | 256           | 64MB
// 32KB    | 512           | 128MB
// 64KB    | 1024          | 256MB
// 256KB   | 4096          | 1GB
// 512KB   | 8192          | 2GB
// 1MB     | 16384         | 4GB
// 4MB     | 65535         | 16GB
// 8MB     | 65535         | 32GB
// 1GB     | 65535         | 4TB

#[cfg(feature = "async")]
pub mod bucket;

pub mod stream;

pub mod entry;

/// A set of common utility functions.
pub mod utils;

pub mod collections;

// pub mod directory;

pub mod hasher;

#[cfg(test)]
pub mod test_utils;

#[cfg(not(feature = "async"))]
pub mod sync;
