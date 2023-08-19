use std::time::Duration;

use simulon::api;
use simulon::simulation::SimulationBuilder;

async fn exec() {
    api::sleep(Duration::from_secs(1_000)).await;
    panic!("This should not be reached.");
}

pub fn main() {
    SimulationBuilder::new(|| api::spawn(exec()))
        .with_nodes(5)
        .set_node_metrics_rate(Duration::ZERO)
        .enable_progress_bar()
        .run(Duration::from_secs(10));
}
