use std::collections::{BTreeSet, HashMap};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use lightning_topology::{build_latency_matrix, suggest_connections_from_latency_matrix};
use plotters::style::full_palette::TEAL_600;
use simulon::latency::ping::ClampNormalDistribution;
use simulon::latency::LatencyProvider;
use simulon::simulation::SimulationBuilder;

use crate::plotting::plot_bar_chart;
use crate::utils::{get_nodes_reached_per_timestep, get_nodes_reached_per_timestep_summary};

mod plotting;
mod setup;
mod utils;

pub fn main() {
    const N: usize = 1500;
    let mut lat_provider =
        simulon::latency::PingDataLatencyProvider::<ClampNormalDistribution>::default();
    lat_provider.init(N);

    let mut latencies = HashMap::new();
    for i in 0..(N - 1) {
        for j in (i + 1)..N {
            let lat = lat_provider.get(i, j);
            latencies.insert((i, j), lat);
        }
    }

    let valid_pubkeys: BTreeSet<usize> = (0..N).collect();
    let (matrix, mappings, _) = build_latency_matrix(usize::MAX, latencies, valid_pubkeys);
    let connections = suggest_connections_from_latency_matrix(0, matrix, &mappings, 9, 8);

    let time = std::time::Instant::now();
    let report = SimulationBuilder::new(|| simulon::api::spawn(setup::exec(N)))
        .with_nodes(N + 1)
        .set_latency_provider(lat_provider)
        .with_state(Arc::new(connections))
        .set_node_metrics_rate(Duration::ZERO)
        .enable_progress_bar()
        .run(Duration::from_secs(120));
    println!("Took {} ms", time.elapsed().as_millis());

    let precision_in_ms = 5;
    let steps_to_num_nodes =
        get_nodes_reached_per_timestep(&report.log.emitted, N, true, precision_in_ms);
    let steps_to_num_nodes = get_nodes_reached_per_timestep_summary(&steps_to_num_nodes);
    let mean_and_std_dev: Vec<(i32, i32)> = steps_to_num_nodes
        .into_iter()
        .map(|s| {
            (
                ((s.mean * 1000.0) as i32),
                ((s.variance.sqrt() * 1000.0) as i32),
            )
        })
        .collect();

    let output_path = PathBuf::from("simulation/plots/percentage_nodes_reached.png");

    plot_bar_chart(
        mean_and_std_dev,
        "Percentage of nodes reached by message per time step",
        &format!("Time steps in {precision_in_ms} [ms]"),
        "Average percentage of nodes reached",
        TEAL_600,
        true,
        false,
        &output_path,
    )
    .unwrap();
    println!("Plot saved to {output_path:?}");
    let mut bytes_sent = 0;
    let mut bytes_recv = 0;
    report.node.iter().for_each(|node| {
        bytes_sent += node.total.bytes_sent;
        bytes_recv += node.total.bytes_received;
    });
    println!("Bytes sent: {bytes_sent}");
    println!("Bytes recv: {bytes_recv}");
}
