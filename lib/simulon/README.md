# Simulon

A network (latency) simulation engine to simulate complex distributed systems at scale. Checkout the [examples](./examples/)
directory to see the usage of the API.
use std::time::Duration;

## Example

```rs
use simulon::{api, simulation::SimulationBuilder};

async fn exec() {
    // Use api::* to do cool stuff. Like connecting to other nodes, listening on a port and
    // more.
}

pub fn main() {
    // Build a simulator with the provided executor function and simulate for
    // 10 simulated seconds.
    SimulationBuilder::new(|| api::spawn(exec()))
        // Specify the number of nodes.
        .with_nodes(10_000)
        // Don't collect metrics for indivisual nodes at each time frame.
        .set_node_metrics_rate(Duration::ZERO)
        .enable_progress_bar()
        .run(Duration::from_secs(10));
}
```
