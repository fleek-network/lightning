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

pub mod capture;

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

#[test]
fn xss() {
    fn human_size(mut s: usize) -> String {
        let mut r = String::new();
        for unit in ["B", "KB", "MB", "GB"] {
            let n = s & ((1 << 10) - 1);
            s >>= 10;
            if n > 0 {
                if r.is_empty() {
                    r = format!("{n}{unit}");
                } else {
                    r = format!("{n}{unit} {r}");
                }
            }
        }
        if s > 0 {
            if r.is_empty() {
                r = format!("{s}TB");
            } else {
                r = format!("{s}TB {r}");
            }
        }
        r
    }

    println!("Tree\t| Entries\t| Max File Size");
    for size in [
        96,
        128,
        256,
        512,
        1024,
        4 * 1024,
        8 * 1024,
        16 * 1024,
        32 * 1024,
        64 * 1024,
        256 * 1024,
        512 * 1024,
        1024 * 1024,
        4 * 1024 * 1024,
        8 * 1024 * 1024,
        1024 * 1024 * 1024,
    ] {
        let hashes = size / 32;
        let entries = (hashes + 1) / 2;
        let fs = entries << 18;
        let dir = entries.min(u16::MAX as usize);
        println!("{}\t| {dir}\t\t| {}", human_size(size), human_size(fs));
    }
}
