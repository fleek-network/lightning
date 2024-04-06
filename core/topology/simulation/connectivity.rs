use std::collections::BTreeMap;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use indicatif::ProgressBar;
use lightning_test_utils::plotting;
use lightning_test_utils::statistics::{get_mean, get_variance};
use rayon::iter::{IntoParallelIterator, ParallelIterator};
use rs_graph::maxflow::pushrelabel;
use rs_graph::IndexGraph;

mod setup;

fn main() {
    let num_trials = 4;
    let num_nodes = [1000, 2000, 5000, 10_000];
    let cluster_sizes = [8, 16, 32, 64, 128];

    // for debugging
    //let num_trials = 1;
    //let num_nodes = [20, 30];
    //let cluster_sizes = [4, 5, 6];

    let pb =
        ProgressBar::new((num_nodes.len() * cluster_sizes.len() * (num_trials as usize)) as u64);

    let data: BTreeMap<usize, BTreeMap<usize, (f64, f64)>> = num_nodes
        .into_par_iter()
        .map(|n| {
            let mut data_cluster_size = BTreeMap::<usize, (f64, f64)>::new();
            for cluster_size in cluster_sizes {
                let mut exp_data = Vec::new();
                for _trial in 0..num_trials {
                    // for each trial we want to sample a different topology
                    let (adj_list, _latencies, _valid_pubkeys) =
                        setup::build_topology(n, cluster_size);
                    let graph = setup::build_graph(&adj_list);

                    let mut non_adj_pairs = Vec::new();
                    for i in 0..(adj_list.len() - 1) {
                        for j in (i + 1)..adj_list.len() {
                            if !adj_list.get(&i).unwrap().contains(&j)
                                && !adj_list.get(&j).unwrap().contains(&i)
                            {
                                non_adj_pairs.push((i, j));
                            }
                        }
                    }

                    // Menger's theorem states that the minimum number of nodes that we need to
                    // remove in order to to disconnect a graph is equal to the
                    // maximum number of disjoint paths between any two
                    // non-adjacent nodes.

                    // We calculate the max flow for all pairs of non-adjacent nodes in the topology
                    // graph. The max flow between two nodes in a graph is equal
                    // to the number of edge disjoint paths between those nodes.
                    let mut max_flow = 0;
                    for (i, j) in non_adj_pairs {
                        let s = graph.id2node(i);
                        let t = graph.id2node(j);
                        let (value, _flow, _mincut) = pushrelabel(&graph, s, t, |_| 1);
                        max_flow = max_flow.max(value);
                    }
                    exp_data.push(max_flow as f64);

                    pb.inc(1);
                }
                let conn_mean = get_mean(&exp_data).unwrap();
                let conn_var = get_variance(&exp_data).unwrap();
                data_cluster_size.insert(cluster_size, (conn_mean, conn_var));
            }
            (n, data_cluster_size)
        })
        .collect();

    let raw_data_path = PathBuf::from("simulation/raw_data/connectivity");
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

    let output_path = PathBuf::from("simulation/plots/connectivity/connectivity_cluster_size.png");
    plotting::line_plot(
        &data,
        "Node connectivity for different cluster sizes",
        "Cluster size",
        "Node Connectivity",
        true,
        false,
        &output_path,
    )
    .unwrap();
    println!("Plot saved to {output_path:?}");
}
