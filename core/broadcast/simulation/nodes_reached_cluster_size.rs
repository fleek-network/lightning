use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use indicatif::ProgressBar;
use lightning_topology::{build_latency_matrix, suggest_connections_from_latency_matrix};
use plotting::line_plot;
use serde::{Deserialize, Serialize};
use simulon::latency::ping::ClampNormalDistribution;
use simulon::latency::LatencyProvider;
use simulon::simulation::SimulationBuilder;
use statrs::distribution::{ContinuousCDF, StudentsT};

use crate::utils::{get_nodes_reached_per_timestep, get_nodes_reached_per_timestep_summary};

mod plotting;
mod setup;
mod utils;

const EPS: f64 = 1e-8;

#[derive(Serialize, Deserialize)]
struct ExperimentData {
    timestep: usize,
    bytes_sent: u64,
    bytes_received: u64,
    //#[allow(unused)]
    //report: Report,
}

pub fn main() {
    // we repeat the experiment to average out the randomness
    let num_trials = 4;
    let num_nodes = [1000, 2000, 5000, 10_000];
    let cluster_sizes = [4, 8, 12, 16, 24, 32];
    let nodes_reached_threshold = 0.9;
    // significance level for t-test when comparing the mean across
    // messages to `nodes_reached_threshold`
    let significance_level = 0.05;
    let raw_data_path = PathBuf::from("simulation/raw_data/");

    let propagation_speed_weight = 0.5;
    assert!((0.0..=1.0).contains(&propagation_speed_weight));

    // for debugging
    //let num_trials = 2;
    //let num_nodes = [1000, 2000];
    //let cluster_sizes = [8, 12];

    let pb =
        ProgressBar::new((num_nodes.len() * cluster_sizes.len() * (num_trials as usize)) as u64);
    println!("Running simulation...");
    // HashMap::<num_nodes, HashMap<cluster_size, Vec<ExperimentData>>>
    let data: HashMap<usize, HashMap<usize, Vec<ExperimentData>>> = num_nodes
        .into_iter()
        .map(|n| {
            let mut data_cluster_size = HashMap::<usize, Vec<ExperimentData>>::new();
            for cluster_size in cluster_sizes {
                for _trial in 0..num_trials {
                    // for each trial we want to sample a different topology
                    let mut lat_provider = simulon::latency::PingDataLatencyProvider::<
                        ClampNormalDistribution,
                    >::default();
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
                    let connections = suggest_connections_from_latency_matrix(
                        0,
                        matrix,
                        &mappings,
                        9,
                        cluster_size,
                    );

                    let report =
                        SimulationBuilder::new(move || simulon::api::spawn(setup::exec(n)))
                            .with_nodes(n + 1)
                            .set_latency_provider(lat_provider)
                            .with_state(Arc::new(connections))
                            .set_node_metrics_rate(Duration::ZERO)
                            .run(Duration::from_secs(120));

                    let steps_to_num_nodes =
                        get_nodes_reached_per_timestep(&report.log.emitted, n, true, 1);
                    let steps_to_num_nodes =
                        get_nodes_reached_per_timestep_summary(&steps_to_num_nodes);

                    for (step_in_millis, s) in steps_to_num_nodes.into_iter().enumerate() {
                        // for debugging
                        //if s.mean >= nodes_reached_threshold {
                        //    let mut bytes_sent = 0;
                        //    let mut bytes_received = 0;
                        //    report.node.iter().for_each(|node| {
                        //        bytes_sent += node.total.bytes_sent;
                        //        bytes_received += node.total.bytes_received;
                        //    });
                        //    data_cluster_size.entry(cluster_size).or_default().push(
                        //        ExperimentData {
                        //            timestep: step_in_millis,
                        //            bytes_sent,
                        //            bytes_received,
                        //            report,
                        //        },
                        //    );
                        //    break;
                        //}

                        let tdist = StudentsT::new(0.0, 1.0, (n - 1) as f64).unwrap();
                        let t = (nodes_reached_threshold - s.mean)
                            / ((s.variance / s.n as f64).sqrt() + EPS);
                        let p = tdist.cdf(t);
                        if p < significance_level {
                            // we reject the null hypothesis:
                            // this is the first time step where the
                            // average number of nodes reached is significantly larger than 90%

                            let mut bytes_sent = 0;
                            let mut bytes_received = 0;
                            report.node.iter().for_each(|node| {
                                bytes_sent += node.total.bytes_sent;
                                bytes_received += node.total.bytes_received;
                            });
                            data_cluster_size.entry(cluster_size).or_default().push(
                                ExperimentData {
                                    timestep: step_in_millis,
                                    bytes_sent,
                                    bytes_received,
                                    //report,
                                },
                            );
                            break;
                        }
                    }

                    pb.inc(1);
                }
            }
            (n, data_cluster_size)
        })
        .collect();

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

    // BTreeMap::<num_nodes, BTreeMap<cluster_size, (mean, variance)>>
    let mut data_timesteps = BTreeMap::<usize, BTreeMap<usize, (f64, f64)>>::new();
    let mut data_bytes = BTreeMap::<usize, BTreeMap<usize, (f64, f64)>>::new();

    let mut timestep_min = usize::MAX;
    let mut timestep_max = usize::MIN;
    let mut mega_bytes_min = f64::MAX;
    let mut mega_bytes_max = f64::MIN;

    for (n, cluster_size_map) in data.iter() {
        for (cluster_size, experiment_data) in cluster_size_map.iter() {
            for d in experiment_data {
                let mega_bytes_transfered =
                    (d.bytes_received as f64) / 1e6 + (d.bytes_sent as f64) / 1e6;
                timestep_min = timestep_min.min(d.timestep);
                timestep_max = timestep_max.max(d.timestep);
                mega_bytes_min = mega_bytes_min.min(mega_bytes_transfered);
                mega_bytes_max = mega_bytes_max.max(mega_bytes_transfered);
            }

            let timesteps_float: Vec<f64> =
                experiment_data.iter().map(|d| d.timestep as f64).collect();
            let mean = utils::get_mean(&timesteps_float).unwrap();
            let variance = utils::get_variance(&timesteps_float).unwrap();
            data_timesteps
                .entry(*n)
                .or_default()
                .insert(*cluster_size, (mean, variance));

            let mega_bytes_float: Vec<f64> = experiment_data
                .iter()
                .map(|d| (d.bytes_received as f64) / 1e6 + (d.bytes_sent as f64) / 1e6)
                .collect();
            let mean = utils::get_mean(&mega_bytes_float).unwrap();
            let variance = utils::get_variance(&mega_bytes_float).unwrap();
            data_bytes
                .entry(*n)
                .or_default()
                .insert(*cluster_size, (mean, variance));
        }
    }

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
        false,
        &output_path,
    )
    .unwrap();
    println!("Plot saved to {output_path:?}");

    let output_path = PathBuf::from("simulation/plots/cluster_size_mb_transfered.png");
    line_plot(
        &data_bytes,
        "Megabytes transfered",
        "Cluster size",
        "Megabytes send + bytes received",
        true,
        false,
        &output_path,
    )
    .unwrap();
    println!("Plot saved to {output_path:?}");

    // Combine the two metrics

    // BTreeMap::<num_nodes, BTreeMap<cluster_size, (mean, variance)>>
    let mut data_timesteps_bytes = BTreeMap::<usize, BTreeMap<usize, (f64, f64)>>::new();
    for (n, cluster_size_map) in data.iter() {
        for (cluster_size, experiment_data) in cluster_size_map.iter() {
            let timesteps_and_bytes: Vec<f64> = experiment_data
                .iter()
                .map(|d| {
                    let timestep = d.timestep as f64;
                    let mega_bytes_transfered =
                        (d.bytes_received as f64) / 1e6 + (d.bytes_sent as f64) / 1e6;

                    let timestep_norm = (timestep - timestep_min as f64)
                        / (timestep_max as f64 - timestep_min as f64);
                    let mega_bytes_transfered_norm = (mega_bytes_transfered - mega_bytes_min)
                        / (mega_bytes_max - mega_bytes_min);
                    timestep_norm * propagation_speed_weight
                        + (1.0 - propagation_speed_weight) * mega_bytes_transfered_norm
                })
                .collect();
            let mean = utils::get_mean(&timesteps_and_bytes).unwrap();
            let variance = utils::get_variance(&timesteps_and_bytes).unwrap();
            data_timesteps_bytes
                .entry(*n)
                .or_default()
                .insert(*cluster_size, (mean, variance));
        }
    }

    let output_path = PathBuf::from("simulation/plots/nodes_reached_and_data_cluster_size.png");
    line_plot(
        &data_timesteps,
        &format!(
            "Average propagation time of a message to reach at least {}% nodes and data transfered",
            (nodes_reached_threshold * 100.0) as u32
        ),
        "Cluster size",
        "Time in ms + data transfered",
        true,
        false,
        &output_path,
    )
    .unwrap();
    println!("Plot saved to {output_path:?}");
}
