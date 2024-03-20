use std::collections::{BTreeSet, HashMap};
use std::hash::Hash;
use std::time::Duration;

use lightning_interfaces::types::Epoch;
use ndarray::{Array, Array2};
use rand::SeedableRng;

use crate::divisive::DivisiveHierarchy;

type LatencyMatrix<K> = (Array2<i32>, HashMap<usize, K>, Option<usize>);

pub enum Connections {
    All(Vec<Vec<usize>>),
    Hierarchy(Vec<Vec<Vec<usize>>>),
}

impl Connections {
    pub fn get(&self, node_index: usize) -> Vec<Vec<usize>> {
        match self {
            Connections::All(conns) => conns.clone(),
            Connections::Hierarchy(conns) => conns[node_index].clone(),
        }
    }
}

/// Build a latency matrix according to the current application state.
/// Returns the matrix, a map of node ids to public keys, and an optional node index for
/// ourselves if we're included in the topology.
pub fn build_latency_matrix<K: Hash + Eq + Copy>(
    our_key: K,
    latencies: HashMap<(K, K), Duration>,
    valid_pubkeys: BTreeSet<K>,
) -> LatencyMatrix<K> {
    let mut max_latency = Duration::ZERO;
    latencies
        .values()
        .for_each(|latency| max_latency = max_latency.max(*latency));
    let max_latency: i32 = max_latency.as_millis().try_into().unwrap_or(i32::MAX);

    let mut matrix = Array::zeros((valid_pubkeys.len(), valid_pubkeys.len()));
    let pubkeys: Vec<(usize, K)> = valid_pubkeys.iter().copied().enumerate().collect();

    let mut our_index = None;
    let mut index_to_pubkey = HashMap::new();
    for (index_lhs, pubkey_lhs) in pubkeys.iter() {
        index_to_pubkey.insert(*index_lhs, *pubkey_lhs);
        if *pubkey_lhs == our_key {
            our_index = Some(*index_lhs);
        }
        for (index_rhs, pubkey_rhs) in pubkeys[index_lhs + 1..].iter() {
            if let Some(latency) = latencies.get(&(*pubkey_lhs, *pubkey_rhs)) {
                let latency: i32 = latency.as_millis().try_into().unwrap_or(i32::MAX);
                matrix[[*index_lhs, *index_rhs]] = latency;
                matrix[[*index_rhs, *index_lhs]] = latency;
            } else if let Some(latency) = latencies.get(&(*pubkey_rhs, *pubkey_lhs)) {
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

pub fn suggest_connections_from_latency_matrix<K: Hash + Eq + Copy>(
    epoch: Epoch,
    matrix: Array2<i32>,
    mappings: &HashMap<usize, K>,
    min_nodes: usize,
    target_k: usize,
) -> Connections {
    // Included in the topology: collect assignments and build output
    if mappings.len() < min_nodes {
        // Fallback to returning all nodes, since we're less than the minimum
        Connections::All(vec![mappings.clone().into_keys().collect()])
    } else {
        let mut rng = rand_chacha::ChaCha8Rng::seed_from_u64(epoch);
        let hierarchy = DivisiveHierarchy::new(&mut rng, &matrix, target_k);
        Connections::Hierarchy(hierarchy.connections())
    }
}

pub fn suggest_connections<K: Hash + Eq + Copy>(
    epoch: Epoch,
    our_key: K,
    latencies: HashMap<(K, K), Duration>,
    valid_pubkeys: BTreeSet<K>,
    min_nodes: usize,
    target_k: usize,
) -> Vec<Vec<K>> {
    let (matrix, mappings, our_index) = build_latency_matrix(our_key, latencies, valid_pubkeys);

    if let Some(our_index) = our_index {
        let connections =
            suggest_connections_from_latency_matrix(epoch, matrix, &mappings, min_nodes, target_k);
        let connections = match &connections {
            Connections::All(connections) => connections,
            Connections::Hierarchy(connections) => &connections[our_index],
        };
        connections
            .iter()
            .map(|ids| ids.iter().map(|idx| mappings[idx]).collect())
            .collect()
    } else {
        // Not in the topology: return all nodes to bootstrap from
        vec![mappings.into_values().collect()]
    }
}
