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

use std::io::{Error, ErrorKind, Result};
use std::path::{Path, PathBuf};

use arrayref::array_ref;
use rand::random;
use tokio::fs::{self, File, OpenOptions};
use tokio::io::{AsyncReadExt, BufWriter};
use triomphe::Arc;

use crate::utils::{to_hex, Digest};

pub mod dir;
pub mod errors;
pub mod file;

pub const HEADER_VERSION: u32 = 1;
pub const HEADER_TYPE_DIR: u8 = 0;
pub const HEADER_TYPE_FILE: u8 = 1;
pub const POSITION_START_HASHES: usize = 9;
pub const POSITION_START_NUM_ENTRIES: usize = POSITION_START_HASHES - 4;

/// An open b3fs bucket which can be used for both reads and writes.
#[derive(Clone)]
pub struct Bucket {
    root: PathBuf,
    // The cache of `$root/{blocks,headers,wal}`
    blocks: PathBuf,
    headers: PathBuf,
    wal: PathBuf,
}

/// Any content from the b3fs blockstore, this content might be either a file or a directory.
pub struct ContentHeader {
    /// Set from the first byte of the file.
    is_file: bool,
    /// The number of entries for this content. For a file this is the number of blocks and for a
    /// directory this is the number of entries.
    ///
    /// If the content is a directory header this is always smaller or equal to u16::MAX.
    num_entries: u32,
    /// The actual header file that is open.
    header_file: Arc<File>,
}

impl Bucket {
    /// Open a bucket at the given path.
    pub async fn open<P: AsRef<Path>>(path: P) -> Result<Self> {
        // turn the root path into an absolute path. this will prevent any bugs from changing the
        // cwd after the bucket is opened.
        let mut root_path = std::env::current_dir()?;
        root_path.push(&path);
        fs::create_dir_all(&root_path).await?;

        let mut blocks_path = root_path.clone();
        blocks_path.push("blocks");
        fs::create_dir(&blocks_path).await;

        let mut headers_path = root_path.clone();
        headers_path.push("headers");
        fs::create_dir(&headers_path).await;

        let mut wal_path = root_path.clone();
        wal_path.push("wal");
        fs::create_dir(&wal_path).await;

        Ok(Self {
            root: root_path,
            blocks: blocks_path,
            headers: headers_path,
            wal: wal_path,
        })
    }

    /// Returns `Ok(true)` if the given content exists in this bucket.
    pub async fn exists(&self, hash: &[u8; 32]) -> Result<bool> {
        let path = self.get_header_path(hash);
        fs::try_exists(&path).await
    }

    /// Open a content with the given provided hash. Returns a [`ContentHeader`] on success.
    pub async fn get(&self, hash: &[u8; 32]) -> Result<ContentHeader> {
        let path = self.get_header_path(hash);
        let mut file = OpenOptions::new().read(true).open(path).await?;
        let mut buf = [0u8; 8];

        // read the first 8 bytes of the file. the first 8 bytes is always the same format for both
        // the directories and files. the second 4 byte i
        let n = file.read_exact(&mut buf).await?;
        debug_assert_eq!(n, 8);

        let version = u32::from_le_bytes(*array_ref![buf, 0, 4]);
        if version > 1 {
            return Err(Error::new(
                ErrorKind::InvalidData,
                "unsupported b3fs header version",
            ));
        }

        let is_file = version == 0;
        let num_entries = u32::from_le_bytes(*array_ref![buf, 4, 4]);
        if !is_file && num_entries > (u16::MAX as u32) {
            return Err(Error::new(
                ErrorKind::InvalidData,
                "too many entries in b3fs directory.",
            ));
        }

        Ok(ContentHeader {
            is_file,
            num_entries,
            header_file: Arc::new(file),
        })
    }

    pub fn get_block_path(&self, counter: u32, hash: &[u8; 32]) -> PathBuf {
        // TODO(qti3e): use the counter in file name.
        let mut path = self.blocks.clone();
        path.push(to_hex(hash).as_str());
        path
    }

    pub fn get_header_path(&self, hash: &[u8; 32]) -> PathBuf {
        let mut path = self.headers.clone();
        path.push(to_hex(hash).as_str());
        path
    }

    pub(crate) fn get_new_wal_path(&self) -> PathBuf {
        let mut path = self.wal.clone();
        let name: [u8; 32] = random();
        path.push(to_hex(&name).as_str());
        path
    }
}

impl ContentHeader {
    /// Returns true if this content is a file.
    pub fn is_file(&self) -> bool {
        self.is_file
    }

    /// Returns true if this content is a directory layout.
    pub fn is_dir(&self) -> bool {
        !self.is_file()
    }

    /// If this content is a file returns a [B3File][file::reader::B3File].
    pub fn into_file(self) -> Option<file::reader::B3File> {
        let f = self.header_file.clone();
        let file = Arc::try_unwrap(f).map(Some).unwrap_or_default();
        if let Some(f) = file {
            self.is_file()
                .then(|| file::reader::B3File::new(self.num_entries, f))
        } else {
            None
        }
    }

    /// If this content is a directory returns a [B3Dir][dir::reader::B3Dir].
    pub fn into_dir(self) -> Option<dir::reader::B3Dir> {
        self.is_dir()
            .then(|| dir::reader::B3Dir::new(self.num_entries, self.header_file))
    }
}

#[cfg(test)]
mod tests {
    use std::env::temp_dir;

    use super::*;
    use crate::hasher::b3::MAX_BLOCK_SIZE_IN_BYTES;

    #[tokio::test]
    async fn open_should_work_multiple_times() {
        let mut temp_dir = temp_dir();
        temp_dir.push("b3fs-open-multiple-times");
        assert!(Bucket::open(&temp_dir).await.is_ok());
        assert!(Bucket::open(&temp_dir).await.is_ok());
        fs::remove_dir_all(&temp_dir);
        assert!(Bucket::open(&temp_dir).await.is_ok());
        fs::remove_dir_all(&temp_dir);
    }

    pub(super) fn get_random_file(size: usize) -> Vec<u8> {
        let mut data = Vec::with_capacity(MAX_BLOCK_SIZE_IN_BYTES);
        for _ in 0..size {
            let d: [u8; 32] = random();
            data.extend(d);
        }
        data
    }
}
