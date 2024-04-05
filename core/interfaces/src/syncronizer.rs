use fdi::BuildGraph;
use lightning_types::Blake3Hash;

use crate::infu_collection::Collection;

#[infusion::service]
pub trait SyncronizerInterface<C: Collection>: BuildGraph + Sized + Send {
    /// Returns the blake3hash of the next checkpoint to load, after
    /// it has already downloaded by the blockstore server.
    async fn next_checkpoint_hash(&self) -> Blake3Hash;
}
