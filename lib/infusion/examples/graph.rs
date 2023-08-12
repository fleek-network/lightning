//! A simple example that prints the graph.

use std::{convert::Infallible, marker::PhantomData};

use infusion::{infu, p};

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

    fn init(_b: &p![::B]) -> Self {
        todo!()
    }
}

impl<C> B for I<C>
where
    C: Collection<B = Self>,
{
    type Collection = C;
}

struct Binding;

impl Collection for Binding {
    type A = I<Binding>;
    type B = I<Binding>;
}

fn main() {
    let graph = Binding::build_graph();
    println!("{:#?}", graph);
}
