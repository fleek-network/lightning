use std::future::Future;
use std::io;
use std::path::PathBuf;

use b3fs::bucket::errors::{CommitError, FeedProofError, InsertError, WriteError};
use b3fs::entry::{BorrowedEntry, OwnedEntry};
use fdi::BuildGraph;
use tokio_stream::StreamExt;

use crate::components::NodeComponents;
use crate::config::ConfigConsumer;
use crate::types::Blake3Hash;

#[interfaces_proc::blank]
pub trait BlockstoreInterface<C: NodeComponents>:
    BuildGraph + Clone + Send + Sync + ConfigConsumer
{
    type FileWriter: FileTrustedWriter;

    type UFileWriter: FileUntrustedWriter;

    type DirWriter: DirTrustedWriter;

    type UDirWriter: DirUntrustedWriter;

    async fn file_writer(&self) -> Result<Self::FileWriter, WriteError>;

    async fn file_untrusted_writer(
        &self,
        root_hash: Blake3Hash,
    ) -> Result<Self::UFileWriter, WriteError>;

    async fn dir_writer(&self, num_entries: usize) -> Result<Self::DirWriter, WriteError>;

    async fn dir_untrusted_writer(
        &self,
        root_hash: Blake3Hash,
        num_entries: usize,
    ) -> Result<Self::UDirWriter, WriteError>;

    /// Returns an open instance of a b3fs bucket.
    fn get_bucket(&self) -> b3fs::bucket::Bucket;

    /// Returns the path to the root directory of the blockstore. The directory layout of
    /// the blockstore is simple.
    fn get_root_dir(&self) -> PathBuf;

    /// Utility function to read an entire file to a vec.
    fn read_all_to_vec(&self, hash: &Blake3Hash) -> impl Future<Output = Option<Vec<u8>>> + Send {
        async {
            let bucket = self.get_bucket();
            let content = bucket.get(hash).await.unwrap();
            let mut result = vec![];
            let blocks = content.blocks();
            if let Some(file) = content.into_file() {
                let mut reader = file.hashtree().await.unwrap();
                for i in 0..blocks {
                    let hash = reader.get_hash(i).await.unwrap().unwrap();
                    let content = bucket.get_block_content(&hash).await.unwrap().unwrap();
                    result.extend_from_slice(&content);
                }
            }
            Some(result)
        }
    }
    /// Utility function to read an entire dir to a vec.
    fn dir_read_all_to_vec(
        &self,
        hash: &Blake3Hash,
    ) -> impl Future<Output = Option<Vec<OwnedEntry>>> + Send {
        async {
            let bucket = self.get_bucket();
            let content = bucket.get(hash).await.unwrap();
            let mut result = vec![];
            if let Some(ref mut dir) = content.into_dir() {
                let mut reader = dir.entries().await.unwrap();
                while let Some(entry) = reader.next().await {
                    let e = entry.unwrap();
                    result.push(e);
                }
            }
            Some(result)
        }
    }
}

/// The interface for the writer to a [`BlockstoreInterface`].
#[interfaces_proc::blank]
pub trait FileUntrustedWriter: FileTrustedWriter {
    /// Write the proof for the buffer.
    async fn feed_proof(&mut self, proof: &[u8]) -> Result<(), FeedProofError>;
}
/// The interface for the writer to a [`BlockstoreInterface`].
#[interfaces_proc::blank]
pub trait FileTrustedWriter: Send + Sync + 'static {
    /// Write the content. If there has been a call to `feed_proof`, an incremental
    /// validation will happen.
    async fn write(&mut self, content: &[u8], last_bytes: bool) -> Result<(), WriteError>;

    /// Finalize the write, try to write all of the content to the file system or any other
    /// underlying storage medium used to implement the [`BlockstoreInterface`].
    async fn commit(self) -> Result<Blake3Hash, CommitError>;

    async fn rollback(self) -> Result<(), io::Error>;
}

/// The interface for the directory writer to a [`BlockstoreInterface`].
#[interfaces_proc::blank]
pub trait DirUntrustedWriter: DirTrustedWriter {
    /// Write the proof for the next entry. Should not be called in the trusted mode.
    async fn feed_proof(&mut self, proof: &[u8]) -> Result<(), FeedProofError>;
}
/// The interface for the directory writer to a [`BlockstoreInterface`].
#[interfaces_proc::blank]
pub trait DirTrustedWriter: Send + Sync + 'static {
    /// Insert the next directory entry. The calls to this method must be in alphabetic order,
    /// based on the name of the entry.
    async fn insert(
        &mut self,
        entry: BorrowedEntry<'_>,
        last_entry: bool,
    ) -> Result<(), InsertError>;

    fn ready_to_commit(&self) -> bool;

    /// Finalize the write, try to write the directory header to the file system.
    async fn commit(self) -> Result<Blake3Hash, CommitError>;

    async fn rollback(self) -> Result<(), io::Error>;
}
