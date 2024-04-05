use std::time::Instant;

use rs_graph::maxflow::pushrelabel;
use rs_graph::IndexGraph;

mod setup;

fn main() {
    let n = 1000;
    let cluster_size = 16;

    let (adj_list, _latencies, _valid_pubkeys) = setup::build_topology(n, cluster_size);
    let graph = setup::build_graph(&adj_list);

    let mut non_adj_pairs = Vec::new();
    for i in 0..(adj_list.len() - 1) {
        for j in (i + 1)..adj_list.len() {
            if !adj_list.get(&i).unwrap().contains(&j) && !adj_list.get(&j).unwrap().contains(&i) {
                non_adj_pairs.push((i, j));
            }
        }
    }

    // Menger's theorem states that the minimum number of nodes that we need to remove in order to
    // to disconnect a graph is equal to the maximum number of disjoint paths between any two
    // non-adjacent nodes.

    // We calculate the max flow for all pairs of non-adjacent nodes in the topology graph.
    // The max flow between two nodes in a graph is equal to the number of edge disjoint paths
    // between those nodes.
    let start = Instant::now();
    let mut max_flow = 0;
    for (i, j) in non_adj_pairs {
        let s = graph.id2node(i);
        let t = graph.id2node(j);
        let (value, _flow, _mincut) = pushrelabel(&graph, s, t, |_| 1);
        max_flow = max_flow.max(value);
    }
    println!("The node connectivity of the graph is: {}", max_flow);
    println!("Took: {:?}", start.elapsed());
}
