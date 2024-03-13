use std::fmt::Write;

use crate::DependencyGraph;

pub fn summary() {}

pub fn dependency_graph(graph: &DependencyGraph, title: &str) -> String {
    let mut res = String::with_capacity(4096);

    writeln!(res, "---").unwrap();
    writeln!(res, "title: {title}").unwrap();
    writeln!(res, "---").unwrap();
    writeln!(res, "stateDiagram-v2").unwrap();
    writeln!(res, "  direction LR").unwrap();

    for (ty, dependencies) in &graph.graph {
        let ty_name = normalize_type_name(ty.name());

        for (_, dep) in dependencies {
            let dep_name = normalize_type_name(dep.name());
            writeln!(res, "{dep_name} --> {ty_name}").unwrap();
        }
    }

    res
}

fn normalize_type_name(mut name: &str) -> &str {
    if let Some(n) = name.find('<') {
        name = &name[0..n];
    }

    while let Some(n) = name.find("::") {
        name = &name[(n + 2)..];
    }

    name
}
