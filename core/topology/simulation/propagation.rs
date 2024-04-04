use std::cmp::Reverse;
use std::collections::{BTreeMap, BTreeSet, BinaryHeap, HashMap};
use std::path::PathBuf;
use std::time::Duration;

use fxhash::FxHashMap;
use indicatif::ProgressBar;
use lightning_test_utils::plotting;
use lightning_test_utils::statistics::{get_mean, get_variance};
use lightning_topology::{build_latency_matrix, suggest_connections_from_latency_matrix};
use simulon::latency::ping::ClampNormalDistribution;
use simulon::latency::LatencyProvider;

type NodeId = usize;
type LatencyMillis = u128;
type Time = u64;

struct Graph {
    nodes: Vec<NodeInfo>,
    queue: BinaryHeap<QueueEntry>,
    current_time: Time,
    max_time: Time,
}

#[derive(Clone, Default, Debug)]
struct NodeInfo {
    connections: FxHashMap<NodeId, LatencyMillis>,
    /// Number of nodes that were reached because we enqueued them.
    nodes_reached: usize,
    /// When this node was visited.
    visited_at: Option<Time>,
    /// The depth this node was visited at.
    visited_at_depth: u8,
}

#[derive(PartialEq, Eq, PartialOrd, Ord)]
struct QueueEntry {
    time: Reverse<Time>,
    destination: NodeId,
    source: NodeId,
    // depth 0 -> source is None.
    depth: u8,
}

impl Graph {
    pub fn new(
        adj_matrix: &BTreeMap<usize, Vec<usize>>,
        latencies: &HashMap<(usize, usize), Duration>,
    ) -> Self {
        let mut nodes = Vec::with_capacity(adj_matrix.len());
        for (node, neighbors) in adj_matrix {
            let connections: FxHashMap<NodeId, LatencyMillis> = neighbors
                .iter()
                .map(|neighbor| {
                    let lat = if node < neighbor {
                        latencies.get(&(*node, *neighbor))
                    } else {
                        latencies.get(&(*neighbor, *node))
                    }
                    .unwrap();
                    (*neighbor, lat.as_millis())
                })
                .collect();
            nodes.push(NodeInfo {
                connections,
                nodes_reached: 0,
                visited_at: None,
                visited_at_depth: 0,
            });
        }

        Self {
            nodes,
            queue: BinaryHeap::new(),
            current_time: 0,
            max_time: 0,
        }
    }

    #[allow(unused)]
    pub fn reset_visit_state(&mut self) {
        self.current_time = 0;
        for node in &mut self.nodes {
            node.visited_at = None;
        }
    }

    #[inline]
    fn process(&mut self, entry: QueueEntry) {
        let current_node = &mut self.nodes[entry.destination];
        if current_node.visited_at.is_some() {
            // skip this node since it has been visited before.
            return;
        }

        // visit
        self.current_time = entry.time.0;
        self.max_time = self.max_time.max(self.current_time);
        current_node.visited_at = Some(self.current_time);
        current_node.visited_at_depth = entry.depth;

        // enqueue connected nodes
        for (&node_id, &latency) in &current_node.connections {
            self.queue.push(QueueEntry {
                time: Reverse(self.current_time + latency as u64),
                destination: node_id,
                source: entry.destination,
                depth: entry.depth + 1,
            });
        }

        // increment the parent by one
        if entry.depth > 0 {
            self.nodes[entry.source].nodes_reached += 1;
        }
    }

    pub fn walk(&mut self, start: NodeId) {
        self.process(QueueEntry {
            time: Reverse(self.current_time),
            destination: start,
            source: 0,
            depth: 0,
        });

        while let Some(entry) = self.queue.pop() {
            self.process(entry);
        }
    }
}

fn main() {
    let num_trials = 4;
    let num_nodes = [1000, 2000, 5000, 10_000];
    let cluster_sizes = [4, 8, 16, 32, 64];

    //let num_nodes = [1000, 2000];
    //let cluster_sizes = [4, 8, 16, 32];

    let pb =
        ProgressBar::new((num_nodes.len() * cluster_sizes.len() * (num_trials as usize)) as u64);
    println!("Running simulation...");
    // HashMap::<num_nodes, HashMap<cluster_size, Vec<ExperimentData>>>
    let data: BTreeMap<usize, BTreeMap<usize, (f64, f64)>> = num_nodes
        .into_iter()
        .map(|n| {
            let mut data_cluster_size = BTreeMap::<usize, (f64, f64)>::new();
            for cluster_size in cluster_sizes {
                let mut propagation_times = Vec::new();
                for _trial in 0..num_trials {
                    // for each trial we want to sample a different topology
                    let mut lat_provider = simulon::latency::PingDataLatencyProvider::<
                        ClampNormalDistribution,
                    >::default();
                    lat_provider.init(n);

                    let valid_pubkeys: BTreeSet<usize> = (0..n).collect();
                    let mut latencies = HashMap::new();
                    for i in 0..(n - 1) {
                        for j in (i + 1)..n {
                            let lat = lat_provider.get(i, j);
                            latencies.insert((i, j), lat);
                        }
                    }

                    let (matrix, mappings, _) =
                        build_latency_matrix(usize::MAX, latencies.clone(), valid_pubkeys.clone());
                    let connections = suggest_connections_from_latency_matrix(
                        0,
                        matrix,
                        &mappings,
                        9,
                        cluster_size,
                    );

                    let adj_matrix: BTreeMap<usize, Vec<usize>> = mappings
                        .into_iter()
                        .map(|(index, key)| {
                            (
                                key,
                                connections
                                    .get(index)
                                    .into_iter()
                                    .flatten()
                                    .collect::<Vec<_>>(),
                            )
                        })
                        .collect();

                    for node in valid_pubkeys {
                        let mut graph = Graph::new(&adj_matrix, &latencies);
                        graph.walk(node);

                        //let num_visited = graph
                        //    .nodes
                        //    .iter()
                        //    .filter(|node| node.visited_at.is_some())
                        //    .count();

                        propagation_times.push(graph.max_time as f64);
                    }

                    pb.inc(1);
                }
                let mean = get_mean(&propagation_times).unwrap();
                let var = get_variance(&propagation_times).unwrap();
                data_cluster_size.insert(cluster_size, (mean, var.sqrt()));
            }
            (n, data_cluster_size)
        })
        .collect();

    let output_path = PathBuf::from("simulation/plots/propagation_speed_cluster_size.png");
    plotting::line_plot(
        &data,
        "Average propagation time of a message to reach all nodes",
        "Cluster size",
        "Time in ms",
        true,
        false,
        &output_path,
    )
    .unwrap();
    println!("Plot saved to {output_path:?}");
}
