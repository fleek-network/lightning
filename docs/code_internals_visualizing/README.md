# **GraphViz's Visualizations Generator**

#### **GraphViz Visualizations Generator for the codebase and crates/modules.**

This tool generates graph diagrams for the whole codebase or separate exact crates/modules.
It can generate graphs of dependencies for modules in a crate, and graphs for code internals inside modules of a crate (package), visualizing interrelations of types, traits (abstract types, interfaces), and functions/methods to them.

It can be used for documentation, onboarding new team-mates, and for analyzing the points of future architectural improvements.

## **Requirements:**

- [Cargo-modules](https://github.com/regexident/cargo-modules)
- [GraphViz](https://www.graphviz.org/documentation/)

This tool requires `cargo-modules` plugin, as a frontend, which generates the output in a `GraphViz`'s dot language:
```bash
cargo install cargo-modules
```

And also requires [GraphViz](https://www.graphviz.org/download/) toolkit (with `dot` graphs drawing tool), as a backend, for drawing graphs from the output of `cargo-modules` in a `GraphViz`'s dot language:
```bash
pacman -Sy graphviz
```

It's recommended to install `xdot` tool as an interactive graph viewer, but it's not required.
```bash
pacman -Sy xdot
```

## **Usage:**

Run bash script from the root directory of the project.

For usage help:
```bash
bash ./docs/code_internals_visualizing/graphs_gen.sh --help
```

For graph visualizing of exact crates/modules:
```bash
bash ./docs/code_internals_visualizing/graphs_gen.sh - "lightning-interfaces lightning-service-executor"
```

To making the graphs for the whole codebase (whole `cargo` workspace crates/packages), using optimal preset of layouts: 
```bash
/usr/bin/time -v bash ./docs/code_internals_visualizing/graphs_gen.sh 2>&1
```
*Note:* This operation will take around 10 minutes.

To making the graphs for the whole codebase (whole `cargo` workspace crates/packages), using full set of `GraphViz` layouts (set of integrated layout algorithms - dot, neato, twopi, circo, fdp, sfdp):
```bash
/usr/bin/time -v bash ./docs/code_internals_visualizing/graphs_gen.sh --full 2>&1
```
*Note:* This operation will take up to 6 hours.

## **Examples of graphs visualizations:**

Graph of dependencies for modules in `lightning-interfaces` crate:
![`lightning-interfaces` crate modules dependencies](graphs_examples/crates_modules/lightning-interfaces/lightning-interfaces.crate_modules.svg)

Graph for code internals inside modules of `lightning-interfaces` crate (package), picturing interrelations of types, traits (abstract types, interfaces), and functions/methods to them:
![`lightning-interfaces` crate types/traits/functions interrelations](graphs_examples/crates_modules/lightning-interfaces/lightning-interfaces.types_traits_fns.svg)

Graph of dependencies for modules in `lightning-service-executor` crate:
![`lightning-service-executor` crate modules dependencies](graphs_examples/crates_modules/lightning-service-executor/lightning-service-executor.crate_modules.svg)

Graph for code internals inside modules of `lightning-service-executor` crate (package), picturing interrelations of types, traits (abstract types, interfaces), and functions/methods to them:
![`lightning-service-executor` crate types/traits/functions interrelations](graphs_examples/crates_modules/lightning-service-executor/lightning-service-executor.types_traits_fns.svg)

*Note:*

The whole set of visualizations (exhaustive full set of code visualizations for `fdi` and `infusion` based branches of codebase, including visualizations for every crate/package and modules in `cargo` workspace, and visualizations using all
available `GraphViz` layout algorithms) will remain in [`docs/graphs` branch in git history](https://github.com/fleek-network/lightning/tree/9c4c9cb1a2a99001e18b34fe989500ab33cb2198/docs/code_internals_visualizing) for any reference.
