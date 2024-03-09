mod event;
mod executor;
// mod ext;
mod extractor;
// mod graph;
mod dyn_method;
mod helpers;
mod impl_tuple;
mod method;
mod provider;
mod ty;
// mod x;

pub use event::Eventstore;
pub use executor::Executor;
// pub use ext::MethodExt;
// pub use graph::DependencyGraph;
pub use method::Method;
pub use provider::{Provider, ProviderGuard, Ref, RefMut};

// #[cfg(test)]
// mod tests;

mod test {
    use crate::{Method, Ref};

    fn expect_method<F, Args>(f: F)
    where
        F: Method<Args>,
    {
    }

    fn x() {
        struct A0;
        struct A1;
        expect_method(|a: &mut A0, b: &A1| {});
        expect_method(|a: &mut A0| {});
        expect_method(|a: &A0| {});
        expect_method(|a: Ref<A0>| {});
    }
}
