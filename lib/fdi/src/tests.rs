use std::borrow::Borrow;
use std::collections::HashMap;
use std::hash::Hash;

use crate::dyn_method::DynMethod;
use crate::{
    consume,
    Bind,
    Consume,
    DependencyGraph,
    Eventstore,
    Method,
    MethodExt,
    Provider,
    Ref,
};

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
    let mut provider = Provider::default();
    graph
        .init_one::<demo_dep::Indexer>(&mut provider)
        .expect("Failed to init.");
    assert!(!provider.contains::<demo_dep::Application>());
    assert!(!provider.contains::<demo_dep::Archive>());
    assert!(!provider.contains::<demo_dep::Blockstore>());
    assert!(provider.contains::<demo_dep::Indexer>());
}

#[test]
fn test_partial_2() {
    let mut graph = demo_dep::graph();
    let mut provider = Provider::default();
    graph
        .init_one::<demo_dep::Blockstore>(&mut provider)
        .expect("Failed to init.");
    assert!(!provider.contains::<demo_dep::Application>());
    assert!(!provider.contains::<demo_dep::Archive>());
    assert!(provider.contains::<demo_dep::Blockstore>());
    // because of _post
    assert!(provider.contains::<demo_dep::Indexer>());
}

#[test]
fn with_value() {
    let provider = Provider::default();
    let value = || String::from("Hello!");
    let value = value.call(&provider);
    assert_eq!(value, "Hello!");

    let provider = Provider::default();
    let value = || String::from("Hello!");
    let value = DynMethod::new(value);
    let value = value.call(&provider);
    assert_eq!(value, "Hello!");

    let mut provider = Provider::default();
    let graph = DependencyGraph::new().with_value(String::from("Hello!"));
    graph.init_all(&mut provider).unwrap();
    assert_eq!(&*provider.get::<String>(), "Hello!");
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

    let mut provider = Provider::default();
    provider.insert(Counter::default());

    graph.init_one::<B>(&mut provider).expect("Failed to init.");
    assert_eq!(provider.get::<Counter>().get("A::_post"), 0);

    graph.init_one::<A>(&mut provider).expect("Failed to init.");
    assert_eq!(provider.get::<Counter>().get("A::_post"), 1);
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

    let mut provider = Provider::default();
    provider.insert(Counter::default());

    graph.init_one::<B>(&mut provider).expect("Failed to init.");
    assert_eq!(provider.get::<Counter>().get("B::_post"), 1);
    assert_eq!(provider.get::<Counter>().get("A::_post"), 1);
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

    let mut provider = Provider::default();
    provider.insert(Counter::default());

    graph.init_one::<A>(&mut provider).expect("Failed to init.");
    assert_eq!(provider.get::<Counter>().get("A::_post"), 1);
    assert_eq!(provider.get::<Counter>().get("A::_start"), 0);
    provider.trigger("_start");
    assert_eq!(provider.get::<Counter>().get("A::_start"), 1);
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

    let mut provider = Provider::default();
    provider.insert(Counter::default());
    graph.init_one::<A>(&mut provider).expect("Failed to init.");
    assert_eq!(provider.get::<Counter>().get("A::_post"), 2);
    assert_eq!(provider.get::<Counter>().get("A::_start"), 0);
    provider.trigger("_start");
    assert_eq!(provider.get::<Counter>().get("A::_start"), 1);
    provider.trigger("_inner");
    assert_eq!(provider.get::<Counter>().get("A::_inner"), 1);
    provider.trigger("_outer");
    assert_eq!(provider.get::<Counter>().get("A::_outer"), 1);
}

#[test]
#[should_panic]
fn init_one_failure_should_panic() {
    #[derive(Default)]
    struct A;

    let mut graph = DependencyGraph::new();
    let mut provider = Provider::default();
    graph.init_one::<A>(&mut provider).unwrap();
}

#[test]
fn depend_on_ref_should_resolve() {
    #[derive(Default)]
    struct A(u32);

    fn method(a: Ref<A>) -> A {
        A(a.0)
    }

    let graph = DependencyGraph::new().with((|| A(17)).to_infallible());
    let mut provider = Provider::default();
    graph.init_all(&mut provider).unwrap();
    let a = method.call(&provider);
    assert_eq!(a.0, 17);
}

#[test]
fn demo_captured() {
    #[derive(Default)]
    struct A(u32);

    impl A {
        pub fn capture(self, value: &String) {
            println!("hello {value}");
        }
    }

    let method_handler = (|a: Consume<A>| A::capture.bind(a.0)).wrap_with(|a| a);

    let graph = DependencyGraph::new()
        .with((|| A(17)).to_infallible().on("start", method_handler))
        .with_value(String::from("World"));

    let mut provider = Provider::default();
    graph.init_all(&mut provider).unwrap();
    provider.trigger("start");
}

#[test]
fn demo_consume() {
    #[derive(Default)]
    struct A(u32);

    impl A {
        pub fn capture(self, value: &String) {
            println!("hello {value}");
        }
    }

    let method_handler = consume(A::capture);
    let graph = DependencyGraph::new()
        .with((|| A(17)).to_infallible().on("start", method_handler))
        .with_value(String::from("World"));

    let mut provider = Provider::default();
    graph.init_all(&mut provider).unwrap();
    provider.trigger("start");
}

#[test]
fn demo_bounded() {
    #[derive(Default)]
    struct A(u32);

    impl A {
        pub fn capture(self, value: &String) {
            println!("hello {value}");
        }
    }

    let graph = DependencyGraph::new()
        .with((|| A(17)).to_infallible().on("start", A::capture.bounded()))
        .with_value(String::from("World"));

    let mut provider = Provider::default();
    graph.init_all(&mut provider).unwrap();
    provider.trigger("start");
}

#[test]
fn async_function_usage() {
    #[derive(Default)]
    struct A(u32);

    impl A {
        async fn hello(&self) -> u32 {
            self.0
        }
    }

    fn expect_method<F, P>(m: F)
    where
        F: Method<P>,
    {
    }

    expect_method(A::hello);
    // expect_method(A::hello.spawn());
    // expect_method(A::hello.block_on());
}
