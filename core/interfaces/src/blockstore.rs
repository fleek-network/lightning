use std::future::Future;
use std::path::PathBuf;

use fdi::BuildGraph;

use crate::components::NodeComponents;
use crate::config::ConfigConsumer;
use crate::types::Blake3Hash;

#[interfaces_proc::blank]
pub trait BlockstoreInterface<C: NodeComponents>:
    BuildGraph + Clone + Send + Sync + ConfigConsumer
{
    /// Returns an open instance of a b3fs bucket.
    fn get_bucket(&self) -> b3fs::bucket::Bucket;

    /// Returns the path to the root directory of the blockstore. The directory layout of
    /// the blockstore is simple.
    fn get_root_dir(&self) -> PathBuf;

    /// Utility function to read an entire file to a vec.
    fn read_all_to_vec(&self, _hash: &Blake3Hash) -> impl Future<Output = Option<Vec<u8>>> + Send {
        async { todo!() }
    }
}
