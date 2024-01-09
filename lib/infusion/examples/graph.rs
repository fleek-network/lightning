#![feature(macro_metavar_expr)]
#![allow(unreachable_code)]

//! A simple example that prints the graph.

use std::marker::PhantomData;

use infusion::{collection, ok, tag};

#[infusion::service]
pub trait A<C: Collection>: Sized {
    fn _init(_b: ::B) {
        ok!(Self::init())
    }

    fn init() -> Self;

    fn hello(&self) {
        println!("Hello from A");
    }
}

#[infusion::service]
pub trait B<C: Collection>: Sized {
    fn hello(&self) {
        println!("Hello from B");
    }
}

collection!([A, B]);

forward!(fn start(this, _x: u8) {
    this.hello();
});

struct I<C: Collection>(PhantomData<C>);

impl<C> A<C> for I<C>
where
    C: Collection<A = Self>,
{
    fn init() -> Self {
        I(PhantomData)
    }
}

impl<C> B<C> for I<C> where C: Collection<B = Self> {}

fn main() {
    let graph = BlankBinding::build_graph();
    println!("{graph:#?}");

    let container = infusion::Container::default()
        .with(
            tag!(BlankBinding::A),
            infusion::Blank::<BlankBinding>::default(),
        )
        .initialize(graph)
        .unwrap();

    start::<BlankBinding>(&container, 10);
}
