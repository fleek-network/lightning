use fdi::BuildGraph;
use lightning_types::Blake3Hash;

use crate::collection::Collection;

#[interfaces_proc::blank]
pub trait SyncronizerInterface<C: Collection>: BuildGraph + Sized + Send + Sync {
    /// Returns the blake3hash of the next checkpoint to load, after
    /// it has already downloaded by the blockstore server.
    /// If it returns None it means the Node is not ever going to checkpoint and should be shutting
    /// down
    #[pending]
    async fn next_checkpoint_hash(&self) -> Option<Blake3Hash>;
}
