use futures::future::{BoxFuture, LocalBoxFuture};
use futures::Future;

use crate::ty::Ty;
use crate::{Eventstore, Provider, ProviderGuard};

/// An internal trait that is implemented for any function-like object that can be called once.
pub trait Method<P> {
    type Output;

    /// The display name of the method. This is used for printing purposes. It defaults
    /// to the name of the output type.
    fn display_name(&self) -> Option<String>;

    /// Return the events that should be registered after this method is invoked.
    fn events(&self) -> Option<Eventstore>;

    /// The parameters this method will ask for from the provider when invoked.
    fn dependencies() -> Vec<Ty>;

    /// Consume and invoke the method.
    fn call(self, provider: &Provider) -> Self::Output;
}

trait AsyncHelper<'a, T: 'a, Args> {
    fn call_helper(self, guard: &'a mut ProviderGuard);
}
impl<'a, F, T: 'a, Fut, A0> AsyncHelper<'a, T, (A0,)> for F
where
    F: FnOnce(&'a A0) -> Fut,
    Fut: Future<Output = T> + 'a,
    A0: 'static,
{
    fn call_helper(self, guard: &'a mut ProviderGuard) {
        let a = guard.get::<A0>();
        (self)(a);
    }
}

impl<F, Fut, T, A0> Method<(T, A0)> for F
where
    F: FnOnce(&A0) -> Fut,
    F: for<'a> AsyncHelper<'a, T, (A0,)>,
    A0: 'static,
{
    type Output = ();

    fn display_name(&self) -> Option<String> {
        todo!()
    }

    fn events(&self) -> Option<Eventstore> {
        todo!()
    }

    fn dependencies() -> Vec<Ty> {
        todo!()
    }

    fn call(self, provider: &Provider) -> Self::Output {
        let mut guard = provider.guard();
        self.call_helper(&mut guard);
        todo!()
    }
}
