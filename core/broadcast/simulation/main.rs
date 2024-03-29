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

    let output_path = PathBuf::from("simulation/images/percentage_nodes_reached.png");

    plot_bar_chart(
        steps_to_num_nodes,
        "Percentage of nodes reached by message per time step",
        &format!("Time steps in {precision_in_ms} [ms]"),
        "Average percentage of nodes reached",
        TEAL_600,
        true,
        &output_path,
    );
}
