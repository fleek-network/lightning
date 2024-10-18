use std::io;

use super::state::uwriter::{DirUWriterCollector, DirUWriterState};
use super::state::{DirState, InnerDirState};
use crate::bucket::{errors, Bucket};
use crate::entry::BorrowedEntry;
use crate::hasher::collector::BufCollector;
use crate::hasher::dir_hasher::DirectoryHasher;
use crate::stream::verifier::{IncrementalVerifier, WithHashTreeCollector};

pub struct UntrustedDirWriter {
    state: DirUWriterState,
}

impl UntrustedDirWriter {
    pub async fn new(
        bucket: &Bucket,
        num_entries: usize,
        hash: [u8; 32],
    ) -> Result<Self, errors::WriteError> {
        let collector = DirUWriterCollector::new(hash);
        let state = InnerDirState::new_with_collector(bucket, num_entries, collector)
            .await
            .map(|state| Self { state })?;
        Ok(state)
    }

    pub async fn feed_proof(&mut self, proof: &[u8]) -> Result<(), errors::FeedProofError> {
        self.state.collector.feed_proof(proof)
    }

    pub async fn insert<'a>(
        &mut self,
        entry: impl Into<BorrowedEntry<'a>>,
    ) -> Result<(), errors::InsertError> {
        self.state.insert_entry(entry.into()).await
    }

    /// Finalize this write and flush the data to the disk.
    pub async fn commit(self) -> Result<[u8; 32], errors::CommitError> {
        self.state.commit().await
    }

    /// Cancel this write and remove anything that this writer wrote to the disk.
    pub async fn rollback(self) -> Result<(), io::Error> {
        self.state.rollback().await
    }
}

#[cfg(test)]
mod tests {

    use super::*;
    use crate::bucket::dir::tests::setup_bucket;
    use crate::bucket::tests::get_random_file;
    use crate::bucket::Bucket;
    use crate::collections::HashTree;
    use crate::entry::{OwnedEntry, OwnedLink};
    use crate::hasher::byte_hasher::Blake3Hasher;
    use crate::hasher::dir_hasher::DirectoryHasher;
    use crate::stream::walker::Mode;
    use crate::test_utils::*;

    fn setup_hasher() -> (Vec<[u8; 32]>, Vec<u8>, [u8; 32]) {
        let mut hasher: Blake3Hasher<Vec<[u8; 32]>> = Blake3Hasher::default();
        let block = get_random_file(8192 * 2);
        hasher.update(&block[..]);

        let (ref mut hashes, root) = hasher.finalize_tree();
        hashes.push(root);
        (hashes.clone(), block, root)
    }

    #[tokio::test]
    async fn test_untrusted_dir_writer_insert() {
        let bucket = setup_bucket().await.unwrap();
        let num_entries = 1;
        let (hashes, block, root) = setup_hasher();
        let hashtree = HashTree::try_from(&hashes).unwrap();

        let mut writer = UntrustedDirWriter::new(&bucket, num_entries, root)
            .await
            .unwrap();

        let entry = OwnedEntry {
            name: "test_file.txt".as_bytes().into(),
            link: OwnedLink::Content(root),
        };
        writer
            .feed_proof(
                hashtree
                    .generate_proof(Mode::from_is_initial(true), 0)
                    .as_slice(),
            )
            .await
            .unwrap();
        let result = writer.insert(&entry).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_untrusted_dir_writer_commit() {
        let bucket = setup_bucket().await.unwrap();
        let num_entries = 1;
        let hash = [0u8; 32];

        let mut writer = UntrustedDirWriter::new(&bucket, num_entries, hash)
            .await
            .unwrap();

        let entry = OwnedEntry {
            name: "test_file.txt".as_bytes().into(),
            link: OwnedLink::Content([1; 32]),
        };
        writer.insert(&entry).await.unwrap();

        let result = writer.commit().await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap().len(), 32);
    }

    #[tokio::test]
    async fn test_untrusted_dir_writer_rollback() {
        let bucket = setup_bucket().await.unwrap();
        let num_entries = 1;
        let hash = [0u8; 32];

        let mut writer = UntrustedDirWriter::new(&bucket, num_entries, hash)
            .await
            .unwrap();

        let entry = OwnedEntry {
            name: "test_file.txt".as_bytes().into(),
            link: OwnedLink::Content([1; 32]),
        };
        writer.insert(&entry).await.unwrap();

        let result = writer.rollback().await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_untrusted_dir_writer_feed_proof() {
        let bucket = setup_bucket().await.unwrap();
        let num_entries = 1;
        let hash = [0u8; 32];

        let mut writer = UntrustedDirWriter::new(&bucket, num_entries, hash)
            .await
            .unwrap();

        let proof = vec![1, 2, 3, 4, 5];
        let result = writer.feed_proof(&proof).await;
        assert!(result.is_ok());
    }
}
