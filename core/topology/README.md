# Fleek Network Topology

## Simulations 

> The following assumes the current directory is `core/topology`

### Generating sample data

The simulations utilize a parser to use the data that can be found at https://wonderproxy.com/blog/a-day-in-the-life-of-the-internet/. There is a script that will automatically pull the raw data, and run the parser against it, which can be run like so:

```bash
./dataset.sh
```

### Topology Report

Run the topology algorithm on the sample data, generating a report showing the network at each depth, and metrics about the lowest level clusters' average latencies.

```bash
cargo run -r --example topology-report
```

### Broadcast Simulation

Run a broadcast simulation on some sampled data, using the topology algorithm to cluster and pair nodes together.

```bash
cargo run -r --example topology-broadcast
```
