use std::cmp::Reverse;
use std::collections::{BTreeMap, BTreeSet, BinaryHeap, HashMap};
use std::path::PathBuf;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use fxhash::FxHashMap;
use indicatif::ProgressBar;
use lightning_test_utils::plotting;
use lightning_test_utils::statistics::{get_mean, get_variance};
use lightning_topology::{build_latency_matrix, suggest_connections_from_latency_matrix};
use rayon::iter::{IntoParallelIterator, ParallelIterator};
use serde::{Deserialize, Serialize};
use simulon::latency::ping::ClampNormalDistribution;
use simulon::latency::LatencyProvider;

type NodeId = usize;
type LatencyMillis = u128;
type Time = u64;

const ADV_SIZE: usize = 38;
const WANT_SIZE: usize = 6;
// the size is 88 bytes w/o payload, but we assume a payload of 512 bytes
const MSG_SIZE: usize = 600;

#[derive(Serialize, Deserialize)]
struct ExperimentData {
    propagation_time: Time,
    bytes_transfered: usize,
}

struct Graph {
    nodes: Vec<NodeInfo>,
    queue: BinaryHeap<QueueEntry>,
    current_time: Time,
    max_time: Time,
    num_edges: usize,
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
        let mut num_edges = 0;
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
            num_edges += connections.len();
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
            num_edges,
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
    let propagation_speed_weight = 0.5;

    let start = Instant::now();

    let pb =
        ProgressBar::new((num_nodes.len() * cluster_sizes.len() * (num_trials as usize)) as u64);
    println!("Running simulation...");
    // HashMap::<num_nodes, HashMap<cluster_size, Vec<ExperimentData>>>
    let data: BTreeMap<usize, BTreeMap<usize, Vec<ExperimentData>>> = num_nodes
        .into_par_iter()
        .map(|n| {
            let mut data_cluster_size = BTreeMap::<usize, Vec<ExperimentData>>::new();
            for cluster_size in cluster_sizes {
                let mut exp_data = Vec::new();
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

                        // estimate bytes sent over the network
                        let num_edges = graph.num_edges;
                        let num_nodes = graph.nodes.len();
                        let bytes_transfered =
                            (num_edges / 2) * ADV_SIZE + num_nodes * (WANT_SIZE + MSG_SIZE);
                        exp_data.push(ExperimentData {
                            propagation_time: graph.max_time,
                            bytes_transfered,
                        });
                    }

                    pb.inc(1);
                }
                data_cluster_size.insert(cluster_size, exp_data);
            }
            (n, data_cluster_size)
        })
        .collect();

    let raw_data_path = PathBuf::from("simulation/raw_data/");
    let raw_data = bincode::serialize(&data).expect("Failed to serialize raw data");
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("Time went backwards")
        .as_millis();

    if !raw_data_path.exists() {
        std::fs::create_dir_all(&raw_data_path).expect("Failed to create directory");
    }
    let raw_data_path = raw_data_path.join(format!("{timestamp}.bin"));
    std::fs::write(&raw_data_path, raw_data).expect("Failed to write raw data to disk");
    println!("Raw data saved to {raw_data_path:?}");

    println!("Took {} ms", start.elapsed().as_millis());

    // BTreeMap::<num_nodes, BTreeMap<cluster_size, (mean, variance)>>
    let mut data_prop_time = BTreeMap::<usize, BTreeMap<usize, (f64, f64)>>::new();
    let mut data_bytes = BTreeMap::<usize, BTreeMap<usize, (f64, f64)>>::new();

    let mut prop_time_min = u64::MAX;
    let mut prop_time_max = u64::MIN;
    let mut mega_bytes_min = f64::MAX;
    let mut mega_bytes_max = f64::MIN;

    for (n, cluster_size_map) in data.iter() {
        for (cluster_size, experiment_data) in cluster_size_map.iter() {
            for d in experiment_data {
                let mega_bytes_transfered = (d.bytes_transfered as f64) / 1e6;
                prop_time_min = prop_time_min.min(d.propagation_time);
                prop_time_max = prop_time_max.max(d.propagation_time);
                mega_bytes_min = mega_bytes_min.min(mega_bytes_transfered);
                mega_bytes_max = mega_bytes_max.max(mega_bytes_transfered);
            }

            let prop_time_float: Vec<f64> = experiment_data
                .iter()
                .map(|d| d.propagation_time as f64)
                .collect();
            let mean = get_mean(&prop_time_float).unwrap();
            let variance = get_variance(&prop_time_float).unwrap();
            data_prop_time
                .entry(*n)
                .or_default()
                .insert(*cluster_size, (mean, variance));

            let mega_bytes_float: Vec<f64> = experiment_data
                .iter()
                .map(|d| (d.bytes_transfered as f64) / 1e6)
                .collect();
            let mean = get_mean(&mega_bytes_float).unwrap();
            let variance = get_variance(&mega_bytes_float).unwrap();
            data_bytes
                .entry(*n)
                .or_default()
                .insert(*cluster_size, (mean, variance));
        }
    }

    let output_path = PathBuf::from("simulation/plots/propagation_time_cluster_size.png");
    plotting::line_plot(
        &data_prop_time,
        "Average propagation time of a message to reach all nodes",
        "Cluster size",
        "Time in ms",
        true,
        false,
        &output_path,
    )
    .unwrap();
    println!("Plot saved to {output_path:?}");

    let output_path = PathBuf::from("simulation/plots/cluster_size_data_transfered.png");
    plotting::line_plot(
        &data_bytes,
        "Megabytes transfered",
        "Cluster size",
        "Megabytes transfered",
        true,
        false,
        &output_path,
    )
    .unwrap();
    println!("Plot saved to {output_path:?}");

    // Combine the two metrics

    // BTreeMap::<num_nodes, BTreeMap<cluster_size, (mean, variance)>>
    let mut data_prop_time_bytes = BTreeMap::<usize, BTreeMap<usize, (f64, f64)>>::new();
    for (n, cluster_size_map) in data.iter() {
        for (cluster_size, experiment_data) in cluster_size_map.iter() {
            let prop_time_and_bytes: Vec<f64> = experiment_data
                .iter()
                .map(|d| {
                    let prop_time = d.propagation_time as f64;
                    let mega_bytes_transfered = (d.bytes_transfered as f64) / 1e6;

                    let prop_time_norm = (prop_time - prop_time_min as f64)
                        / (prop_time_max as f64 - prop_time_min as f64);
                    let mega_bytes_transfered_norm = (mega_bytes_transfered - mega_bytes_min)
                        / (mega_bytes_max - mega_bytes_min);
                    prop_time_norm * propagation_speed_weight
                        + (1.0 - propagation_speed_weight) * mega_bytes_transfered_norm
                })
                .collect();
            let mean = get_mean(&prop_time_and_bytes).unwrap();
            let variance = get_variance(&prop_time_and_bytes).unwrap();
            data_prop_time_bytes
                .entry(*n)
                .or_default()
                .insert(*cluster_size, (mean, variance));
        }
    }

    let output_path = PathBuf::from("simulation/plots/propagation_time_and_data_cluster_size.png");
    plotting::line_plot(
        &data_prop_time_bytes,
        "Average propagation time of a message to reach all nodes + data transfered",
        "Cluster size",
        "Time in ms + data transfered",
        true,
        false,
        &output_path,
    )
    .unwrap();
    println!("Plot saved to {output_path:?}");
}
