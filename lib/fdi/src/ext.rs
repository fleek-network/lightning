use futures::Future;

use crate::{helpers, Method};

pub trait MethodExt<P>: Sized + Method<P> {
    #[inline(always)]
    fn with_display_name(self, name: &'static str) -> impl Method<P, Output = Self::Output> {
        helpers::display_name(name, self)
    }

    #[inline(always)]
    fn to_infallible(self) -> impl Method<P, Output = anyhow::Result<Self::Output>> {
        helpers::to_infalliable(self)
    }

    #[inline(always)]
    fn map<M, U>(self, transform: M) -> impl Method<P, Output = U>
    where
        M: FnOnce(Self::Output) -> U,
        U: 'static,
    {
        helpers::map(self, transform)
    }

    #[inline(always)]
    fn flatten<B>(self) -> impl Method<(P, B), Output = <Self::Output as Method<B>>::Output>
    where
        Self::Output: Method<B>,
    {
        helpers::flatten(self)
    }

    #[inline(always)]
    fn on<H, Y>(self, event: &'static str, handler: H) -> impl Method<P, Output = Self::Output>
    where
        H: Method<Y>,
    {
        helpers::on(self, event, handler)
    }

    #[inline(always)]
    fn spawn(self) -> impl Method<P, Output = ()>
    where
        Self: 'static + Method<P> + Sized,
        Self::Output: Future,
    {
        helpers::spawn(self)
    }

    #[inline(always)]
    fn block_on(self) -> impl Method<P, Output = <Self::Output as Future>::Output>
    where
        Self: 'static + Method<P> + Sized,
        Self::Output: Future,
    {
        helpers::block_on(self)
    }
}

impl<F, P> MethodExt<P> for F where F: Method<P> {}
