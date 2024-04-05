use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::time::Duration;

use lightning_topology::{build_latency_matrix, suggest_connections_from_latency_matrix};
use rs_graph::{Buildable, Builder, VecGraph};
use simulon::latency::ping::ClampNormalDistribution;
use simulon::latency::LatencyProvider;

pub type Topology = (
    BTreeMap<usize, HashSet<usize>>,
    HashMap<(usize, usize), Duration>,
    BTreeSet<usize>,
);

pub fn build_topology(num_nodes: usize, cluster_size: usize) -> Topology {
    let mut lat_provider =
        simulon::latency::PingDataLatencyProvider::<ClampNormalDistribution>::default();
    lat_provider.init(num_nodes);

    let valid_pubkeys: BTreeSet<usize> = (0..num_nodes).collect();
    let mut latencies = HashMap::new();
    for i in 0..(num_nodes - 1) {
        for j in (i + 1)..num_nodes {
            let lat = lat_provider.get(i, j);
            latencies.insert((i, j), lat);
        }
    }

    let (matrix, mappings, _) =
        build_latency_matrix(usize::MAX, latencies.clone(), valid_pubkeys.clone());
    let connections =
        suggest_connections_from_latency_matrix(0, matrix, &mappings, 9, cluster_size);

    let adj_list: BTreeMap<usize, HashSet<usize>> = mappings
        .into_iter()
        .map(|(index, key)| {
            (
                key,
                connections
                    .get(index)
                    .into_iter()
                    .flatten()
                    .collect::<HashSet<_>>(),
            )
        })
        .collect();
    (adj_list, latencies, valid_pubkeys)
}

pub fn build_graph(adj_list: &BTreeMap<usize, HashSet<usize>>) -> VecGraph<usize> {
    let mut b = VecGraph::<usize>::new_builder();
    let mut nodes = HashMap::new();
    for node in adj_list.keys() {
        nodes.insert(node, b.add_node());
    }

    for (node, neighbors) in adj_list {
        for neighbor in neighbors {
            b.add_edge(*nodes.get(&node).unwrap(), *nodes.get(&neighbor).unwrap());
        }
    }
    b.into_graph()
}
