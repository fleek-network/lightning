//! Implementation of the B3FS Bucket (on-disk) API.
//!
//! This module provides the implementation of [Bucket] and various other primitives which can be
//! used for interacting with the file system, that is both in read-only and writer mode. A bucket
//! is created and opened on a location within the actualt file system and has a root path where
//! all of the contents will be written to.
//!
//! The layout of that directory looks like this:
//!
//! ```txt
//! /ROOT/
//!     ./blocks/
//!         ./[blockHash]
//!     ./headers/
//!         ./[contentHash]
//!     ./wal/
//!         ./[randomUUIDs]
//! ```
//!
//! A bucket can hold both files and directories, a directory is a collection of other files or
//! some symbolic links. Any content hash can be either for a file or a directory, in other words
//! the headers files which we store (like `./root/headers/xxx`) can be the description of either.
//!
//! It is only when we open a header and read the first few bytes that we can know if we are
//! dealing with a file or a directory. So due to that [Bucket::get] returns a general
//! [ContentHeader] which can then be turned into either a [File][1] or a [Dir][2] during the
//! runtime.
//!
//! When it comes to inserting new content into the bucket there is usually 2 modes of writing:
//!
//! 1. Trusted: In this mode we are writing the content from a source we already trust and due to
//!    that we do not care about incrementally verifying the bytes. There is no proof needed for
//!    entries.
//!
//! 2. Untrusted: In the untrusted mode we are getting data from an untrusted source which does give
//!    us proofs for the content and we verify these proofs on each step and we can terminate the
//!    insertion as soon as we get a bad byte (chunk).
//!
//! It should be noted that both [File][1] and [Dir][2] represent immutable objects which can no
//! longer be modified. Unlike the standard FS APIs in order to create/write a file or directory
//! a different set of APIs should be used and once the result is written to the disk it is sealed.
//!
//! [1]: file::reader::File
//! [2]: dir::reader::Dir

use std::io::Result;
use std::path::{Path, PathBuf};

use crate::utils::Digest;

pub mod dir;
pub mod file;

pub struct Bucket {
    root: PathBuf,
}

pub struct ContentHeader {}

impl Bucket {
    /// Open a bucket at the given path.
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self> {
        todo!()
    }

    pub async fn get(&self, hash: &[u8; 32]) -> Result<Option<ContentHeader>> {
        todo!()
    }

    pub fn get_block_path(&self, counter: u32, hash: &[u8; 32]) -> PathBuf {
        todo!()
    }

    pub fn get_header_path(&self, hash: &[u8; 32]) -> PathBuf {
        todo!()
    }
}

impl ContentHeader {
    pub fn is_file(&self) -> bool {
        todo!()
    }

    pub fn is_dir(&self) -> bool {
        !self.is_file()
    }

    pub fn into_file(self) -> Option<file::reader::File> {
        todo!()
    }

    pub fn into_dir(self) -> Option<dir::reader::Dir> {
        todo!()
    }
}
