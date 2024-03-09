use futures::Future;

use crate::{helpers, Method};

pub trait MethodExt<'a, P>: Sized + Method<'a, P> {
    #[inline(always)]
    fn with_display_name(self, name: &'static str) -> impl Method<'a, P, Output = Self::Output> {
        helpers::display_name(name, self)
    }

    #[inline(always)]
    fn to_infallible(self) -> impl Method<'a, P, Output = anyhow::Result<Self::Output>> {
        helpers::to_infalliable(self)
    }

    #[inline(always)]
    fn map<M, U>(self, transform: M) -> impl Method<'a, P, Output = U>
    where
        M: FnOnce(Self::Output) -> U,
        U: 'static,
    {
        helpers::map(self, transform)
    }

    #[inline(always)]
    fn on<H, Y>(self, event: &'static str, handler: H) -> impl Method<'a, P, Output = Self::Output>
    where
        H: Method<'a, Y>,
    {
        helpers::on(self, event, handler)
    }

    #[inline(always)]
    fn spawn(self) -> impl Method<'a, P, Output = ()>
    where
        Self: 'static + Method<'a, P> + Sized,
        Self::Output: Future,
    {
        helpers::spawn(self)
    }

    #[inline(always)]
    fn block_on(self) -> impl Method<'a, P, Output = <Self::Output as Future>::Output>
    where
        Self: 'static + Method<'a, P> + Sized,
        Self::Output: Future,
    {
        helpers::block_on(self)
    }
}

impl<'a, F, P> MethodExt<'a, P> for F where F: Method<'a, P> {}
