use lightning_interfaces::infu_collection::{BlankBinding, Collection};

fn main() {
    let graph = BlankBinding::build_graph();
    println!("{}", graph.mermaid("Fleek Network Dependency Graph"));
}
