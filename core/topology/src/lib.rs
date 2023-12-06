pub mod clustering;
pub mod config;
pub mod divisive;
pub mod pairing;

#[cfg(test)]
mod tests;

use std::collections::{BTreeSet, HashMap};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use async_trait::async_trait;
pub use config::Config;
use divisive::DivisiveHierarchy;
use fleek_crypto::NodePublicKey;
use lightning_interfaces::infu_collection::{c, Collection};
use lightning_interfaces::{
    ApplicationInterface,
    ConfigConsumer,
    SyncQueryRunnerInterface,
    TopologyInterface,
};
use ndarray::{Array, Array2};
use rand::SeedableRng;
use tracing::info;

pub struct Topology<C: Collection> {
    inner: Arc<TopologyInner<c![C::ApplicationInterface::SyncExecutor]>>,
}

impl<C: Collection> Clone for Topology<C> {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
        }
    }
}

struct TopologyInner<Q: SyncQueryRunnerInterface + 'static> {
    query: Q,
    our_public_key: NodePublicKey,
    // TODO(qti3e): Use ArcSwap instead.
    current_peers: Mutex<Arc<Vec<Vec<NodePublicKey>>>>,
    // TODO(qti3e): Use a single AtomicU64 instead.
    current_epoch: Mutex<u64>,
    target_k: usize,
    min_nodes: usize,
}

impl<Q: SyncQueryRunnerInterface> TopologyInner<Q> {
    /// Build a latency matrix according to the current application state.
    /// Returns the matrix, a map of node ids to public keys, and an optional node index for
    /// ourselves if we're included in the topology.
    fn build_latency_matrix(&self) -> (Array2<i32>, HashMap<usize, NodePublicKey>, Option<usize>) {
        let latencies = self.query.get_latencies();
        let valid_pubkeys: BTreeSet<NodePublicKey> = self
            .query
            .get_node_registry(None)
            .into_iter()
            .map(|node_info| node_info.public_key)
            .collect();

        let mut max_latency = Duration::ZERO;
        latencies
            .values()
            .for_each(|latency| max_latency = max_latency.max(*latency));
        let max_latency: i32 = max_latency.as_millis().try_into().unwrap_or(i32::MAX);

        let mut matrix = Array::zeros((valid_pubkeys.len(), valid_pubkeys.len()));
        let pubkeys: Vec<(usize, NodePublicKey)> =
            valid_pubkeys.iter().copied().enumerate().collect();

        let mut our_index = None;
        let mut index_to_pubkey = HashMap::new();
        for (index_lhs, pubkey_lhs) in pubkeys.iter() {
            index_to_pubkey.insert(*index_lhs, *pubkey_lhs);
            if *pubkey_lhs == self.our_public_key {
                our_index = Some(*index_lhs);
            }
            for (index_rhs, pubkey_rhs) in pubkeys[index_lhs + 1..].iter() {
                if let Some(latency) = latencies.get(&(*pubkey_lhs, *pubkey_rhs)) {
                    let latency: i32 = latency.as_millis().try_into().unwrap_or(i32::MAX);
                    matrix[[*index_lhs, *index_rhs]] = latency;
                    matrix[[*index_rhs, *index_lhs]] = latency;
                } else {
                    matrix[[*index_lhs, *index_rhs]] = max_latency;
                    matrix[[*index_rhs, *index_lhs]] = max_latency;
                }
            }
        }

        (matrix, index_to_pubkey, our_index)
    }

    fn suggest_connections(&self) -> Arc<Vec<Vec<NodePublicKey>>> {
        let epoch = self.query.get_epoch();
        let mut current = self.current_peers.lock().expect("failed to acquire lock");
        let mut current_epoch = self.current_epoch.lock().expect("failed to acquire lock");

        // TODO(qti3e): This computation can be heavy. Right now only broadcast is calling this
        // function and we can put the call to `suggest_connections` in a blocking thread on the
        // caller side. But I regret having the topology poll based. It was a mistake.
        //
        // We should have the topology as a reactive source of data so we can put this heavy
        // computation on a different thread here from within the origin of the computation.

        // if the epoch has changed, or the object has been newly initialized
        if *current_epoch < epoch || *current_epoch == u64::MAX {
            let (matrix, mappings, our_index) = self.build_latency_matrix();

            *current = if let Some(our_index) = our_index {
                // Included in the topology: collect assignments and build output
                if mappings.len() < self.min_nodes {
                    // Fallback to returning all nodes, since we're less than the minimum
                    info!("All nodes connect to each other");
                    vec![mappings.into_values().collect()]
                } else {
                    info!("Form hierarchical clusters");
                    let mut rng = rand_chacha::ChaCha8Rng::seed_from_u64(epoch);
                    let hierarchy = DivisiveHierarchy::new(&mut rng, &matrix, self.target_k);

                    hierarchy.connections()[our_index]
                        .iter()
                        .map(|ids| ids.iter().map(|idx| mappings[idx]).collect())
                        .collect()
                }
            } else {
                // Not in the topology: return all nodes to bootstrap from
                vec![mappings.into_values().collect()]
            }
            .into();

            *current_epoch = epoch;
        }

        current.clone()
    }
}

#[async_trait]
impl<C: Collection> TopologyInterface<C> for Topology<C> {
    fn init(
        config: Self::Config,
        our_public_key: NodePublicKey,
        query_runner: c!(C::ApplicationInterface::SyncExecutor),
    ) -> anyhow::Result<Self> {
        let inner = TopologyInner {
            target_k: config.testing_target_k,
            min_nodes: config.testing_min_nodes,
            query: query_runner,
            current_epoch: Mutex::new(u64::MAX),
            current_peers: Mutex::new(Arc::new(Vec::new())),
            our_public_key,
        };
        Ok(Self {
            inner: Arc::new(inner),
        })
    }

    fn suggest_connections(&self) -> Arc<Vec<Vec<NodePublicKey>>> {
        self.inner.suggest_connections()
    }
}

impl<C: Collection> ConfigConsumer for Topology<C> {
    type Config = Config;

    const KEY: &'static str = "topology";
}
