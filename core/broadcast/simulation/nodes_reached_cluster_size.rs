use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use lightning_topology::{build_latency_matrix, suggest_connections_from_latency_matrix};
use plotting::line_plot;
use simulon::latency::ping::ClampNormalDistribution;
use simulon::latency::LatencyProvider;
use simulon::simulation::SimulationBuilder;
use statrs::distribution::{ContinuousCDF, StudentsT};

use crate::utils::{get_nodes_reached_per_timestep, get_nodes_reached_per_timestep_summary};

mod plotting;
mod setup;
mod utils;

const EPS: f64 = 1e-8;

struct ExperimentData {
    timestep: usize,
    bytes_sent: u64,
    bytes_received: u64,
}

pub fn main() {
    // we repeat the experiment to average out the randomness
    let num_trials = 5;
    let num_nodes = [1000, 2000, 5000, 10_000];
    let cluster_sizes = [4, 8, 12, 16, 20, 24, 28, 32];
    let nodes_reached_threshold = 0.9;
    // significance level for t-test when comparing the mean across
    // messages to `nodes_reached_threshold`
    let significance_level = 0.05;

    // HashMap::<num_nodes, HashMap<cluster_size, Vec<ExperimentData>>>
    let mut data = HashMap::<usize, HashMap<usize, Vec<ExperimentData>>>::new();
    for n in num_nodes {
        for cluster_size in cluster_sizes {
            for _trial in 0..num_trials {
                // for each trial we want to sample a different topology
                let mut lat_provider =
                    simulon::latency::PingDataLatencyProvider::<ClampNormalDistribution>::default();
                lat_provider.init(n);

                let mut latencies = HashMap::new();
                for i in 0..(n - 1) {
                    for j in (i + 1)..n {
                        let lat = lat_provider.get(i, j);
                        latencies.insert((i, j), lat);
                    }
                }

                let valid_pubkeys: BTreeSet<usize> = (0..n).collect();
                let (matrix, mappings, _) =
                    build_latency_matrix(usize::MAX, latencies, valid_pubkeys);
                let connections =
                    suggest_connections_from_latency_matrix(0, matrix, &mappings, 9, cluster_size);

                let report = SimulationBuilder::new(move || simulon::api::spawn(setup::exec(n)))
                    .with_nodes(n + 1)
                    .set_latency_provider(lat_provider)
                    .with_state(Arc::new(connections))
                    .set_node_metrics_rate(Duration::ZERO)
                    .enable_progress_bar()
                    .run(Duration::from_secs(120));

                let steps_to_num_nodes =
                    get_nodes_reached_per_timestep(&report.log.emitted, n, true, 1);
                let steps_to_num_nodes =
                    get_nodes_reached_per_timestep_summary(&steps_to_num_nodes);

                for (step_in_millis, s) in steps_to_num_nodes.into_iter().enumerate() {
                    let tdist = StudentsT::new(0.0, 1.0, (n - 1) as f64).unwrap();
                    let t = (nodes_reached_threshold - s.mean)
                        / ((s.variance / s.n as f64).sqrt() + EPS);
                    let p = tdist.cdf(t);
                    if p < significance_level {
                        // we reject the null hypothesis: this is the first time step where the
                        // average number of nodes reached is significantly larger than 90%

                        let mut bytes_sent = 0;
                        let mut bytes_received = 0;
                        report.node.iter().for_each(|node| {
                            bytes_sent += node.total.bytes_sent;
                            bytes_received += node.total.bytes_received;
                        });
                        data.entry(n)
                            .or_default()
                            .entry(cluster_size)
                            .or_default()
                            .push(ExperimentData {
                                timestep: step_in_millis,
                                bytes_sent,
                                bytes_received,
                            });
                        break;
                    }
                }
            }
        }
    }

    //// BTreeMap::<num_nodes, BTreeMap<cluster_size, (mean, variance)>>
    let mut data_timesteps = BTreeMap::<usize, BTreeMap<usize, (f64, f64)>>::new();
    let mut data_bytes = BTreeMap::<usize, BTreeMap<usize, (f64, f64)>>::new();

    for (n, cluster_size_map) in data.into_iter() {
        for (cluster_size, experiment_data) in cluster_size_map.into_iter() {
            let timesteps_float: Vec<f64> =
                experiment_data.iter().map(|d| d.timestep as f64).collect();
            let mean = utils::get_mean(&timesteps_float).unwrap();
            let variance = utils::get_variance(&timesteps_float).unwrap();
            data_timesteps
                .entry(n)
                .or_default()
                .insert(cluster_size, (mean, variance));

            let bytes_float: Vec<f64> = experiment_data
                .iter()
                .map(|d| (d.bytes_sent + d.bytes_received) as f64)
                .collect();
            let mean = utils::get_mean(&bytes_float).unwrap();
            let variance = utils::get_variance(&bytes_float).unwrap();
            data_bytes
                .entry(n)
                .or_default()
                .insert(cluster_size, (mean, variance));
        }
    }

    //let mut data_avg = BTreeMap::<usize, BTreeMap<usize, (f64, f64)>>::new();
    //let mut map1 = BTreeMap::new();
    //map1.insert(4, (100.0, 100.0));
    //map1.insert(8, (80.0, 7.0));
    //map1.insert(16, (50.0, 5.0));
    //data_avg.insert(1000, map1);

    //let mut map2 = BTreeMap::new();
    //map2.insert(4, (120.0, 12.0));
    //map2.insert(8, (100.0, 16.0));
    //map2.insert(16, (60.0, 8.0));
    //data_avg.insert(2000, map2);

    //let min_x = 4;
    //let max_x = 16;
    //let min_y = 0.0;
    //let max_y = 200.0;

    let output_path = PathBuf::from("simulation/plots/nodes_reached_cluster_size.png");
    line_plot(
        &data_timesteps,
        &format!(
            "Average propagation time of a message to reach at least {}% nodes",
            (nodes_reached_threshold * 100.0) as u32
        ),
        "Cluster size",
        "Time in ms",
        true,
        &output_path,
    )
    .unwrap();

    let output_path = PathBuf::from("simulation/plots/cluster_size_bytes_sent_recv.png");
    line_plot(
        &data_timesteps,
        "Bytes sent and received",
        "Cluster size",
        "Bytes send + bytes received",
        true,
        &output_path,
    )
    .unwrap();

    //let time = std::time::Instant::now();

    //println!("Took {} ms", time.elapsed().as_millis());

    //let precision_in_ms = 5;
    //let steps_to_num_nodes =
    //    get_nodes_reached_per_timestep(&report.log.emitted, N, true, precision_in_ms);
    //let steps_to_num_nodes = get_nodes_reached_per_timestep_summary(&steps_to_num_nodes);

    //let output_path = PathBuf::from("simulation/images/percentage_nodes_reached.png");

    //plot_bar_chart(
    //    steps_to_num_nodes,
    //    "Percentage of nodes reached by message per time step",
    //    &format!("Time steps in {precision_in_ms} [ms]"),
    //    "Average percentage of nodes reached",
    //    TEAL_600,
    //    true,
    //    &output_path,
    //);
    //println!("Plot saved to {output_path:?}");
    //let mut bytes_sent = 0;
    //let mut bytes_recv = 0;
    //report.node.iter().for_each(|node| {
    //    bytes_sent += node.total.bytes_sent;
    //    bytes_recv += node.total.bytes_received;
    //});
    //println!("Bytes sent: {bytes_sent}");
    //println!("Bytes recv: {bytes_recv}");
}
