use fdi::BuildGraph;
use lightning_types::Blake3Hash;

use crate::components::NodeComponents;

#[interfaces_proc::blank]
pub trait IndexerInterface<C: NodeComponents>: BuildGraph + Clone + Send + Sync + Sized {
    async fn register(&self, cid: Blake3Hash);

    async fn unregister(&self, cid: Blake3Hash);
}
