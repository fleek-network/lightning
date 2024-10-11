use std::num::NonZeroU32;
use std::os::unix::fs::FileExt;
use std::path::{Path, PathBuf};

use bytes::BytesMut;
use rand::random;
use serde::Serialize as _;
use tokio::fs::{File, OpenOptions};
use tokio::io::{self, AsyncWriteExt, BufWriter};

use super::*;
use crate::bucket::dir::phf::{HasherState, PhfGenerator};
use crate::bucket::{errors, Bucket};
use crate::entry::{BorrowedEntry, BorrowedLink};
use crate::hasher::b3::MAX_BLOCK_SIZE_IN_BYTES;
use crate::hasher::collector::BufCollector;
use crate::hasher::dir_hasher::DirectoryHasher;
use crate::hasher::HashTreeCollector;
use crate::utils;

/// Final state should be usefull for both writers: trusted and untrusted
pub(crate) struct DirWriterState {
    bucket: Bucket,
    phf_generator: PhfGenerator,
    next_position: u32,
    hasher: DirectoryHasher,
    temp_file_path: PathBuf,
    header_file: HeaderFile,
}

impl DirWriterState {
    pub(crate) async fn new(bucket: &Bucket, num_entries: usize) -> Result<Self, io::Error> {
        let phf_generator = PhfGenerator::new(num_entries);
        let temp_file_path = bucket.get_new_wal_path();
        tokio::fs::create_dir_all(&temp_file_path).await?;
        let header_file = HeaderFile::from_wal_path(&temp_file_path, num_entries).await?;
        let hasher = DirectoryHasher::default();
        Ok(Self {
            bucket: bucket.clone(),
            phf_generator,
            next_position: 0,
            hasher,
            temp_file_path,
            header_file,
        })
    }

    pub(crate) async fn insert_entry<'b>(
        &mut self,
        borrowed_entry: impl Into<BorrowedEntry<'b>>,
    ) -> Result<(), errors::InsertError> {
        let borrowed_entry: BorrowedEntry<'_> = borrowed_entry.into();
        let mut bytes = match borrowed_entry.link {
            BorrowedLink::Content(content) => content,
            BorrowedLink::Path(path) => path,
        };
        let i = self.next_position;
        let pos = unsafe { NonZeroU32::new_unchecked(i) };
        self.phf_generator.push(bytes, pos);
        self.next_position += borrowed_entry.name.len() as u32;
        self.hasher.insert(borrowed_entry);
        self.header_file.insert_entry(borrowed_entry).await?;
        Ok(())
    }

    pub(crate) async fn commit(mut self) -> Result<[u8; 32], errors::CommitError> {
        let (root_hash, tree) = self.hasher.finalize();
        let hash_tree = self.phf_generator.finalize();
        self.header_file
            .commit(&self.bucket, &root_hash, tree, [0; 32].to_vec(), hash_tree)
            .await?;

        Ok(root_hash)
    }

    pub(crate) async fn rollback(self) -> Result<(), io::Error> {
        tokio::fs::remove_dir_all(self.temp_file_path).await
    }
}
