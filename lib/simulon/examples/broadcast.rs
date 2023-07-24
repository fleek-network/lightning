use std::time::Duration;

use simulon::{api, simulation::SimulationBuilder};

async fn run() {}

pub fn main() {
    SimulationBuilder::new(|| api::spawn(run()))
        .with_nodes(2_000)
        .build()
        .run(Duration::from_secs(120));
}
