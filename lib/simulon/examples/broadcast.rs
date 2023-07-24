use std::time::Duration;

use simulon::{api, simulation::SimulationBuilder};

async fn run() {
    let id = api::RemoteAddr::whoami();

    if *id % 5 == 0 {
        api::sleep(Duration::from_secs(1)).await;
        api::emit("scope-1");
    } else {
        api::emit("scope-1");
    }
}

pub fn main() {
    let report = SimulationBuilder::new(|| api::spawn(run()))
        .with_nodes(2_000)
        .build()
        .run(Duration::from_secs(120));

    println!("{:#?}", report.log);
}
