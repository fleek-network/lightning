use fdi::BuildGraph;
use lightning_types::Blake3Hash;

use crate::infu_collection::Collection;

#[infusion::service]
pub trait IndexerInterface<C: Collection>: BuildGraph + Clone + Send + Sync + Sized {
    async fn register(&self, cid: Blake3Hash);

    async fn unregister(&self, cid: Blake3Hash);
}
