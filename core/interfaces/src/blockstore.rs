use std::future::Future;
use std::path::PathBuf;

use fdi::BuildGraph;

use crate::collection::Collection;
use crate::config::ConfigConsumer;
use crate::types::Blake3Hash;

#[interfaces_proc::blank]
pub trait BlockstoreInterface<C: Collection>:
    BuildGraph + Clone + Send + Sync + ConfigConsumer
{
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
}
