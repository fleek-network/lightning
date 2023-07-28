pub mod clustering;
mod config;
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
use fleek_crypto::NodePublicKey;
use freek_interfaces::{ConfigConsumer, SyncQueryRunnerInterface, TopologyInterface};
use ndarray::{Array, Array2};

pub struct Topology<Q: SyncQueryRunnerInterface> {
    #[allow(dead_code)]
    query: Q,
}

impl<Q: SyncQueryRunnerInterface> Topology<Q> {
    #[allow(dead_code)]
    fn build_latency_matrix(&self) -> (Array2<i32>, HashMap<usize, NodePublicKey>) {
        let latencies = self.query.get_latencies();
        let latency_count = latencies.len();
        let mut latency_map: HashMap<NodePublicKey, HashMap<NodePublicKey, Duration>> =
            HashMap::new();
        let mut pubkeys = BTreeSet::new();
        let mut latency_sum = Duration::ZERO;
        for ((pubkey_lhs, pubkey_rhs), latency) in latencies {
            pubkeys.insert(pubkey_lhs);
            pubkeys.insert(pubkey_rhs);
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

        let mut matrix = Array::zeros((pubkeys.len(), pubkeys.len()));
        for (index_lhs, pubkey_lhs) in pubkeys.iter().enumerate() {
            for (index_rhs, pubkey_rhs) in pubkeys.iter().enumerate() {
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
        let index_to_pubkey: HashMap<usize, NodePublicKey> =
            pubkeys.into_iter().enumerate().collect();
        (matrix, index_to_pubkey)
    }
}

#[async_trait]
impl<Q: SyncQueryRunnerInterface> TopologyInterface for Topology<Q> {
    type SyncQuery = Q;

    async fn init(
        _config: Self::Config,
        _our_public_key: NodePublicKey,
        query_runner: Self::SyncQuery,
    ) -> anyhow::Result<Self> {
        Ok(Self {
            query: query_runner,
        })
    }

    fn suggest_connections(&self) -> Arc<Vec<Vec<NodePublicKey>>> {
        todo!()
    }
}

impl<Q: SyncQueryRunnerInterface> ConfigConsumer for Topology<Q> {
    type Config = Config;

    const KEY: &'static str = "TOPOLOGY";
}
