pub mod clustering;
pub mod config;
pub mod divisive;
pub mod pairing;

#[cfg(test)]
mod tests;

use std::{
    collections::{BTreeSet, HashMap},
    sync::Arc,
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
    current_peers: Arc<Vec<Vec<NodePublicKey>>>,
    current_epoch: u64,
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

        let latency_count = latencies.len();
        let mut latency_map: HashMap<NodePublicKey, HashMap<NodePublicKey, Duration>> =
            HashMap::new();
        let mut latency_sum = Duration::ZERO;
        for ((pubkey_lhs, pubkey_rhs), latency) in latencies {
            if !valid_pubkeys.contains(&pubkey_lhs) || !valid_pubkeys.contains(&pubkey_rhs) {
                continue;
            }

            latency_sum += latency;
            let opposite_dir_latency = latency_map
                .get(&pubkey_rhs)
                .and_then(|latency_row| latency_row.get(&pubkey_lhs));

            let latency = if let Some(opp_latency) = opposite_dir_latency {
                // If a latency measurement for the opposite direction exists, we use the average
                // of both latency measurements.
                let avg_latency = (latency + *opp_latency) / 2;
                latency_map
                    .entry(pubkey_rhs)
                    .or_insert(HashMap::new())
                    .insert(pubkey_lhs, avg_latency);
                avg_latency
            } else {
                latency
            };
            latency_map
                .entry(pubkey_lhs)
                .or_insert(HashMap::new())
                .insert(pubkey_rhs, latency);
        }
        let mean_latency = latency_sum / latency_count as u32; // why do we have to cast to u32?
        let mean_latency: i32 = mean_latency.as_micros().try_into().unwrap_or(i32::MAX);

        let mut matrix = Array::zeros((valid_pubkeys.len(), valid_pubkeys.len()));
        for (index_lhs, pubkey_lhs) in valid_pubkeys.iter().enumerate() {
            for (index_rhs, pubkey_rhs) in valid_pubkeys.iter().enumerate() {
                if index_lhs != index_rhs {
                    matrix[[index_lhs, index_rhs]] = mean_latency;
                    matrix[[index_rhs, index_lhs]] = mean_latency;
                    if let Some(latency) = latency_map
                        .get(pubkey_lhs)
                        .and_then(|latency_row| latency_row.get(pubkey_rhs))
                    {
                        let latency: i32 = latency.as_micros().try_into().unwrap_or(i32::MAX);
                        matrix[[index_lhs, index_rhs]] = latency;
                        matrix[[index_rhs, index_lhs]] = latency;
                    }
                    if let Some(latency) = latency_map
                        .get(pubkey_rhs)
                        .and_then(|latency_row| latency_row.get(pubkey_lhs))
                    {
                        let latency: i32 = latency.as_micros().try_into().unwrap_or(i32::MAX);
                        matrix[[index_lhs, index_rhs]] = latency;
                        matrix[[index_rhs, index_lhs]] = latency;
                    }
                }
            }
        }
        let mut our_index = None;
        let index_to_pubkey: HashMap<usize, NodePublicKey> = valid_pubkeys
            .into_iter()
            .enumerate()
            .map(|(index, pubkey)| {
                if pubkey == self.our_public_key {
                    our_index = Some(index);
                }
                (index, pubkey)
            })
            .collect();
        (matrix, index_to_pubkey, our_index)
    }
}

#[async_trait]
impl<Q: SyncQueryRunnerInterface> TopologyInterface for Topology<Q> {
    type SyncQuery = Q;

    async fn init(
        config: Self::Config,
        our_public_key: NodePublicKey,
        query_runner: Self::SyncQuery,
    ) -> anyhow::Result<Self> {
        Ok(Self {
            target_k: config.testing_target_k,
            min_nodes: config.testing_min_nodes,
            query: query_runner,
            current_epoch: u64::MAX,
            current_peers: Arc::new(Vec::new()),
            our_public_key,
        })
    }

    fn suggest_connections(&mut self) -> Arc<Vec<Vec<NodePublicKey>>> {
        let epoch = self.query.get_epoch();

        // if it's the initial epoch or the epoch has changed
        if epoch == u64::MAX || epoch > self.current_epoch {
            let (matrix, mappings, our_index) = self.build_latency_matrix();

            self.current_peers = if let Some(our_index) = our_index {
                // Included in the topology: collect assignments and build output

                if mappings.len() < self.min_nodes {
                    // Fallback to returning all nodes, since we're less than the minimum
                    Arc::new(vec![mappings.into_values().collect()])
                } else {
                    let mut rng = rand_chacha::ChaCha8Rng::seed_from_u64(epoch);
                    let hierarchy = DivisiveHierarchy::new(&mut rng, &matrix, self.target_k);

                    // TODO: add depth support
                    Arc::new(vec![
                        hierarchy.assignments()[our_index]
                            .iter()
                            .map(|idx| mappings[idx])
                            .collect(),
                    ])
                }
            } else {
                // Not in the topology: return all nodes to bootstrap from
                Arc::new(vec![mappings.into_values().collect()])
            };

            self.current_epoch = epoch;
        }

        // return the current peers
        self.current_peers.clone()
    }
}

impl<Q: SyncQueryRunnerInterface> ConfigConsumer for Topology<Q> {
    type Config = Config;

    const KEY: &'static str = "topology";
}
