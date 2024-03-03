use crate::{Container, DependencyGraph, Registry};

mod demo_dep {
    use crate::{DependencyGraph, Eventstore, Named};

    pub struct Application;
    pub struct QueryRunner;
    impl Application {
        pub fn new(_store: &Blockstore) -> Self {
            Application
        }

        pub fn get_query_runner(&self) -> QueryRunner {
            QueryRunner
        }
    }

    pub struct Archive;
    impl Archive {
        pub fn new(_q: &QueryRunner, _b: &Blockstore) -> Self {
            Archive
        }
    }

    pub struct Blockstore;
    impl Blockstore {
        pub fn new(event: &mut Eventstore) -> Self {
            event.on("_post", |_this: &mut Self, _indexer: &Indexer| {
                //
            });
            Blockstore
        }
    }

    pub struct Indexer;
    impl Indexer {
        pub fn new() -> Self {
            Indexer
        }
    }

    pub fn graph() -> DependencyGraph {
        DependencyGraph::new()
            .with_infallible(Named::new("App", Application::new))
            .with_infallible(Named::new(
                "App::QueryRunner",
                Application::get_query_runner,
            ))
            .with_infallible(Archive::new)
            .with_infallible(Blockstore::new)
            .with_infallible(Indexer::new)
    }
}

#[test]
fn test_partial_01() {
    let mut graph = demo_dep::graph();
    let mut registry = Registry::default();
    graph
        .init_one::<demo_dep::Indexer>(&mut registry)
        .expect("Failed to init.");
    assert!(!registry.contains::<demo_dep::Application>());
    assert!(!registry.contains::<demo_dep::Archive>());
    assert!(!registry.contains::<demo_dep::Blockstore>());
    assert!(registry.contains::<demo_dep::Indexer>());
}

#[test]
fn test_partial_2() {
    let mut graph = demo_dep::graph();
    let mut registry = Registry::default();
    graph
        .init_one::<demo_dep::Blockstore>(&mut registry)
        .expect("Failed to init.");
    assert!(!registry.contains::<demo_dep::Application>());
    assert!(!registry.contains::<demo_dep::Archive>());
    assert!(registry.contains::<demo_dep::Blockstore>());
    // because of _post
    assert!(registry.contains::<demo_dep::Indexer>());
}

#[test]
fn depending_on_container() {
    struct A;
    struct B;
    fn new_b(_a: &Container<A>) -> B {
        B
    }

    let mut graph = DependencyGraph::new()
        .with_infallible(|| A)
        .with_infallible(new_b);

    let mut registry = Registry::default();
    graph.init_one::<B>(&mut registry).expect("Failed to init.");
}
