//! A simple example that prints the graph.

use std::{convert::Infallible, marker::PhantomData};

use infusion::{infu, p, tag};

pub trait A: Sized {
    infu!(A, {
        fn init(b: B) {
            Result::<Self, Infallible>::Ok(Self::init(b))
        }

        fn post() {}
    });

    fn init(b: &p![::B]) -> Self;
}

pub trait B: Sized {
    infu!(B @ Input);
}

infu!(@Collection [
    A,
    B
]);

struct I<C: Collection>(PhantomData<C>);

impl<C> A for I<C>
where
    C: Collection<A = Self>,
{
    type Collection = C;

    infu!(impl {
        fn post() {
            println!("Running post initialization for A");
        }
    });

    fn init(_b: &p![::B]) -> Self {
        I(PhantomData)
    }
}

impl<C> B for I<C>
where
    C: Collection<B = Self>,
{
    type Collection = C;

    infu!(impl {
        fn post() {
            println!("Running post initialization for B");
        }
    });
}

struct Binding;

impl Collection for Binding {
    type A = I<Binding>;
    type B = I<Binding>;
}

fn main() {
    let graph = Binding::build_graph();
    println!("{graph:#?}");

    let _container = infusion::Container::default()
        .with(tag!(I<Binding> as B), I::<Binding>(PhantomData))
        .initialize(graph)
        .unwrap();
}
