#![feature(macro_metavar_expr)]

use std::marker::PhantomData;

use infusion::infu;

#[infusion::service]
pub trait A<C: Collection> {}

#[infusion::service]
pub trait B<C: Collection> {}

infu!(@Collection [
    A,
    B
]);

struct I<C: Collection>(PhantomData<C>);
impl<C: Collection> A<C> for I<C> {}

partial!(X {
    A = I<Self>;
});

fn main() {}
