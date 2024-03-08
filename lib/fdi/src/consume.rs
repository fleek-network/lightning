use std::marker::PhantomData;

use crate::Method;

pub struct Consume<F, T, P, O> {
    pub(crate) f: F,
    _p: PhantomData<(F, T, P, O)>,
}

// /// Turns any function that takes an owned value as the first argument into a valid [`Method`].
// ///
// /// # Example
// ///
// /// ```
// /// use fdi::*;
// ///
// /// #[derive(Default)]
// /// struct A;
// ///
// /// impl A {
// ///     fn start(self) {
// ///         //   ^-- The goal is to have access to `self`.
// ///         // The function could also take other parameters.
// ///     }
// /// }
// ///
// /// let mut graph =
// ///     DependencyGraph::new().with(A::default.to_infallible().on("start", consume(A::start)));
// /// let mut registry = Registry::default();
// /// graph.init_one::<A>(&mut registry).unwrap();
// /// registry.trigger("start");
// /// ```
// pub fn consume<F, T, P, O>(f: F) -> impl Method<T, (O, P)>
// where
//     Consume<F, T, P, O>: Method<T, (O, P)>,
// {
//     Consume::<F, T, P, O> { f, _p: PhantomData }
// }
