use std::collections::BTreeMap;
use std::path::PathBuf;

use indicatif::ProgressBar;
use lightning_test_utils::plotting;
use lightning_test_utils::statistics::{get_mean, get_variance};
use rayon::iter::{IntoParallelIterator, ParallelIterator};

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

                    for (_node, neighbors) in adj_list {
                        exp_data.push(neighbors.len() as f64);
                    }

                    pb.inc(1);
                }
                let conn_mean = get_mean(&exp_data).unwrap();
                let conn_var = get_variance(&exp_data).unwrap();
                data_cluster_size.insert(cluster_size, (conn_mean, conn_var));
            }
            (n, data_cluster_size)
        })
        .collect();

    let output_path = PathBuf::from("simulation/plots/connections/connections_cluster_size.png");
    plotting::line_plot(
        &data,
        "Number of connections per node for different cluster sizes",
        "Cluster size",
        "Average Number of Connections",
        true,
        false,
        &output_path,
    )
    .unwrap();
    println!("Plot saved to {output_path:?}");
}
