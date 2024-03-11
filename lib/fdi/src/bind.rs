use crate::{Consume, Method, MethodExt};

/// A function with the first argument to it already captured.
pub struct Captured<Arg, F> {
    pub(crate) arg: Arg,
    pub(crate) fun: F,
}

/// The bind trait could be used to bind the first argument of a function to a certain value.
pub trait Bind<Arg, Args>: Sized {
    type Output: 'static;
    /// Bind the given first arg to the function. Returning a Method that would only depend on the
    /// rest of the arguments.
    fn bind(self, arg: Arg) -> Captured<Arg, Self>;
}

/// Return a method that consumes the first argument. Useful for when you have something like:
/// `F::fun(self, ...)`.
pub fn consume<F, Arg, Args>(f: F) -> impl Method<(), Output = F::Output>
where
    Arg: 'static,
    F: Bind<Arg, Args>,
    Captured<Arg, F>: Method<Args, Output = F::Output>,
{
    (|cap: Consume<Arg>| cap.0).wrap_with(|a| f.bind(a))
}
