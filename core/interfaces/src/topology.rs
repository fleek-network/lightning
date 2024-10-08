use std::sync::Arc;

use fdi::BuildGraph;
use fleek_crypto::NodePublicKey;
use tokio::sync::watch;

use crate::components::NodeComponents;

/// The algorithm used for clustering our network and dynamically creating a network topology.
/// This clustering is later used in other parts of the codebase when connection to other nodes
/// is required. The gossip layer is an example of a component that can feed the data this
/// algorithm generates.
#[interfaces_proc::blank]
pub trait TopologyInterface<C: NodeComponents>: BuildGraph + Sized + Send + Sync {
    /// Get a receiver that will periodically receive the new list of connections that our current
    /// node must connect to. This list will be sent after the epoch changes, but can also be sent
    /// more frequently.
    ///
    /// The list of connections is a 2-dimensional array, the first dimension determines the
    /// closeness of the nodes, the further items are the outer layer of the connections.
    #[blank = watch::channel(Arc::new(vec![])).1]
    fn get_receiver(&self) -> watch::Receiver<Arc<Vec<Vec<NodePublicKey>>>>;
}
