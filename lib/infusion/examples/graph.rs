#![feature(macro_metavar_expr)]

//! A simple example that prints the graph.

use std::marker::PhantomData;

use infusion::{tag, collection, ok};

#[infusion::service]
pub trait A<C: Collection>: Sized {
    fn _init(_b: ::B) {
        ok!(todo!())
    }
}

#[infusion::service]
pub trait B<C: Collection>: Sized {}

collection!([
    A,
    B
]);

struct I<C: Collection>(PhantomData<C>);

impl<C> A<C> for I<C>
where
    C: Collection<A = Self>,
{
}

impl<C> B<C> for I<C>
where
    C: Collection<B = Self>,
{
}

fn main() {
    let graph = BlankBinding::build_graph();
    println!("{graph:#?}");

    let _container = infusion::Container::default()
        .with(tag!(BlankBinding :: A), infusion::Blank::<BlankBinding>::default())
        .initialize(graph)
        .unwrap();
}
