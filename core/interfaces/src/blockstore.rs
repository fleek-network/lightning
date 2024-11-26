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

    fn get_block(
        &self,
        block: u32,
        hash: &Blake3Hash,
    ) -> impl Future<Output = Option<Vec<u8>>> + Send {
        let block_path = self.get_bucket().get_block_path(block, hash);
        async {
            let exists = tokio::fs::try_exists(block_path).await;
            None
        }
    }

    /// Utility function to read an entire file to a vec.
    fn read_all_to_vec(&self, _hash: &Blake3Hash) -> impl Future<Output = Option<Vec<u8>>> + Send {
        async { todo!() }
    }
}
