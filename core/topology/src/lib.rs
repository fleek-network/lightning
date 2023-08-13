pub mod clustering;
pub mod config;
pub mod divisive;
pub mod pairing;

#[cfg(test)]
mod tests;

use std::{
    collections::{BTreeSet, HashMap},
    sync::{Arc, Mutex},
    time::Duration,
};

use async_trait::async_trait;
pub use config::Config;
use divisive::DivisiveHierarchy;
use fleek_crypto::NodePublicKey;
use lightning_interfaces::{ConfigConsumer, SyncQueryRunnerInterface, TopologyInterface};
use ndarray::{Array, Array2};
use rand::SeedableRng;

pub struct Topology<Q: SyncQueryRunnerInterface> {
    query: Q,
    our_public_key: NodePublicKey,
    current_peers: Mutex<Arc<Vec<Vec<NodePublicKey>>>>,
    current_epoch: Mutex<u64>,
    target_k: usize,
    min_nodes: usize,
}

impl<Q: SyncQueryRunnerInterface> Topology<Q> {
    fn build_latency_matrix(&self) -> (Array2<i32>, HashMap<usize, NodePublicKey>, Option<usize>) {
        let latencies = self.query.get_latencies();
        let valid_pubkeys: BTreeSet<NodePublicKey> = self
            .query
            .get_node_registry()
            .into_iter()
            .map(|node_info| node_info.public_key)
            .collect();

        let mut latency_sum = Duration::ZERO;
        latencies
            .values()
            .for_each(|latency| latency_sum += *latency);
        let mean_latency = latency_sum / latencies.len() as u32; // why do we have to cast to u32?
        let mean_latency: i32 = mean_latency.as_micros().try_into().unwrap_or(i32::MAX);

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
                    let latency: i32 = latency.as_micros().try_into().unwrap_or(i32::MAX);
                    matrix[[*index_lhs, *index_rhs]] = latency;
                    matrix[[*index_rhs, *index_lhs]] = latency;
                } else {
                    matrix[[*index_lhs, *index_rhs]] = mean_latency;
                    matrix[[*index_rhs, *index_lhs]] = mean_latency;
                }
            }
        }

        (matrix, index_to_pubkey, our_index)
    }
}

#[async_trait]
impl<Q: SyncQueryRunnerInterface> TopologyInterface for Topology<Q> {
    type SyncQuery = Q;

    fn init(
        config: Self::Config,
        our_public_key: NodePublicKey,
        query_runner: Self::SyncQuery,
    ) -> anyhow::Result<Self> {
        Ok(Self {
            target_k: config.testing_target_k,
            min_nodes: config.testing_min_nodes,
            query: query_runner,
            current_epoch: Mutex::new(u64::MAX),
            current_peers: Mutex::new(Arc::new(Vec::new())),
            our_public_key,
        })
    }

    fn suggest_connections(&self) -> Arc<Vec<Vec<NodePublicKey>>> {
        let epoch = self.query.get_epoch();
        let mut current = self.current_peers.lock().expect("failed to acquire lock");
        let mut current_epoch = self.current_epoch.lock().expect("failed to acquire lock");

        // if it's the initial epoch or the epoch has changed
        if *current_epoch == u64::MAX || *current_epoch < epoch {
            let (matrix, mappings, our_index) = self.build_latency_matrix();

            *current = if let Some(our_index) = our_index {
                // Included in the topology: collect assignments and build output
                if mappings.len() < self.min_nodes {
                    // Fallback to returning all nodes, since we're less than the minimum
                    vec![mappings.into_values().collect()]
                } else {
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

impl<Q: SyncQueryRunnerInterface> ConfigConsumer for Topology<Q> {
    type Config = Config;

    const KEY: &'static str = "topology";
}
