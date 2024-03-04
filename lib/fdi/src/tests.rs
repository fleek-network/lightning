use std::borrow::Borrow;
use std::collections::HashMap;
use std::hash::Hash;

use crate::method::DynMethod;
use crate::{consume, Container, DependencyGraph, Eventstore, Method, MethodExt, Registry};

mod demo_dep {
    use crate::ext::MethodExt;
    use crate::{DependencyGraph, Eventstore};

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
            .with_infallible(Application::new.with_display_name("App"))
            .with_infallible(Application::get_query_runner.with_display_name("App::QueryRunner"))
            .with_infallible(Archive::new)
            .with_infallible(Blockstore::new)
            .with_infallible(Indexer::new)
    }
}

#[derive(Default)]
struct Counter {
    counter: HashMap<String, usize>,
}

impl Counter {
    pub fn add(&mut self, key: impl Into<String>) {
        let key = key.into();
        *self.counter.entry(key).or_default() += 1;
    }

    pub fn get<Q: ?Sized>(&self, k: &Q) -> usize
    where
        String: Borrow<Q>,
        Q: Hash + Eq,
    {
        *self.counter.get(k).unwrap_or(&0)
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

#[test]
fn with_value() {
    let registry = Registry::default();
    let value = || String::from("Hello!");
    let value = value.call(&registry);
    assert_eq!(value, "Hello!");

    let registry = Registry::default();
    let value = || String::from("Hello!");
    let value = DynMethod::new(value);
    let value = value.call(&registry).downcast::<String>().unwrap();
    assert_eq!(*value, "Hello!");

    let mut registry = Registry::default();
    let graph = DependencyGraph::new().with_value(String::from("Hello!"));
    graph.init_all(&mut registry).unwrap();
    assert_eq!(&*registry.get::<String>(), "Hello!");
}

#[test]
fn post_should_be_fired() {
    struct A;
    struct B;

    fn new_a(store: &mut Eventstore) -> A {
        store.on("_post", |counter: &mut Counter| {
            counter.add("A::_post");
        });

        A
    }

    fn new_b() -> B {
        B
    }

    let mut graph = DependencyGraph::new()
        .with_infallible(new_a)
        .with_infallible(new_b);

    let mut registry = Registry::default();
    registry.insert(Counter::default());

    graph.init_one::<B>(&mut registry).expect("Failed to init.");
    assert_eq!(registry.get::<Counter>().get("A::_post"), 0);

    graph.init_one::<A>(&mut registry).expect("Failed to init.");
    assert_eq!(registry.get::<Counter>().get("A::_post"), 1);
}

#[test]
fn post_should_resolve_unmet_dep() {
    struct A;
    struct B;

    fn new_a(store: &mut Eventstore) -> A {
        store.on("_post", |counter: &mut Counter| {
            counter.add("A::_post");
        });
        A
    }

    fn new_b(store: &mut Eventstore) -> B {
        store.on("_post", |counter: &mut Counter, _a: &A| {
            counter.add("B::_post");
        });
        B
    }

    let mut graph = DependencyGraph::new()
        .with_infallible(new_a)
        .with_infallible(new_b);

    let mut registry = Registry::default();
    registry.insert(Counter::default());

    graph.init_one::<B>(&mut registry).expect("Failed to init.");
    assert_eq!(registry.get::<Counter>().get("B::_post"), 1);
    assert_eq!(registry.get::<Counter>().get("A::_post"), 1);
}

#[test]
fn basic_with_events_should_work() {
    #[derive(Default)]
    struct A;

    let mut graph = DependencyGraph::new().with(
        A::default
            .to_infallible()
            .on("_post", |counter: &mut Counter| {
                counter.add("A::_post");
            })
            .on("_start", |counter: &mut Counter| {
                counter.add("A::_start");
            }),
    );

    let mut registry = Registry::default();
    registry.insert(Counter::default());
    graph.init_one::<A>(&mut registry).expect("Failed to init.");
    assert_eq!(registry.get::<Counter>().get("A::_post"), 1);
    assert_eq!(registry.get::<Counter>().get("A::_start"), 0);
    registry.trigger("_start");
    assert_eq!(registry.get::<Counter>().get("A::_start"), 1);
}

#[test]
fn nested_with_events_should_work() {
    #[derive(Default)]
    struct A;

    let mut graph = DependencyGraph::new().with(
        A::default
            .to_infallible()
            .on("_post", |counter: &mut Counter| {
                counter.add("A::_post");
            })
            .on("_start", |counter: &mut Counter| {
                counter.add("A::_start");
            })
            .on("_inner", |counter: &mut Counter| {
                counter.add("A::_inner");
            })
            .on("_post", |counter: &mut Counter| {
                counter.add("A::_post");
            })
            .on("_outer", |counter: &mut Counter| {
                counter.add("A::_outer");
            }),
    );

    let mut registry = Registry::default();
    registry.insert(Counter::default());
    graph.init_one::<A>(&mut registry).expect("Failed to init.");
    assert_eq!(registry.get::<Counter>().get("A::_post"), 2);
    assert_eq!(registry.get::<Counter>().get("A::_start"), 0);
    registry.trigger("_start");
    assert_eq!(registry.get::<Counter>().get("A::_start"), 1);
    registry.trigger("_inner");
    assert_eq!(registry.get::<Counter>().get("A::_inner"), 1);
    registry.trigger("_outer");
    assert_eq!(registry.get::<Counter>().get("A::_outer"), 1);
}

#[test]
#[should_panic]
fn init_one_failure_should_panic() {
    #[derive(Default)]
    struct A;

    let mut graph = DependencyGraph::new();
    let mut registry = Registry::default();
    graph.init_one::<A>(&mut registry).unwrap();
}

#[test]
fn consume_usage() {
    #[derive(Default)]
    struct A;

    impl A {
        fn start(self, counter: &mut Counter) {
            counter.add("start");
        }
    }

    let mut graph =
        DependencyGraph::new().with(A::default.to_infallible().on("start", consume(A::start)));
    let mut registry = Registry::default();
    registry.insert(Counter::default());
    graph.init_one::<A>(&mut registry).unwrap();
    registry.trigger("start");
    assert_eq!(registry.get::<Counter>().get("start"), 1);
}
