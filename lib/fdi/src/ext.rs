use futures::Future;

use crate::{helpers, Executor, Method};

pub trait MethodExt<P>: Sized + Method<P> {
    #[inline(always)]
    fn to_infallible(self) -> impl Method<P, Output = anyhow::Result<Self::Output>> {
        helpers::map::map(self, Ok)
    }

    #[inline(always)]
    fn map_method<M, U>(self, transform: M) -> impl Method<P, Output = U>
    where
        M: FnOnce(Self::Output) -> U,
        U: 'static,
    {
        helpers::map::map(self, transform)
    }

    #[inline(always)]
    fn with_event_handler<H, Y>(
        self,
        event: &'static str,
        handler: H,
    ) -> impl Method<P, Output = Self::Output>
    where
        H: Method<Y>,
    {
        helpers::on::on(self, event, handler)
    }

    #[inline(always)]
    fn wrap_with<W, OutW, ArgsOutW>(self, wrapper: W) -> impl Method<(), Output = OutW::Output>
    where
        W: FnOnce(Self::Output) -> OutW,
        OutW: Method<ArgsOutW>,
    {
        helpers::wrap_with::wrap_with(self, wrapper)
    }

    #[inline(always)]
    fn wrap_with_block_on(self) -> impl Method<P, Output = <Self::Output as Future>::Output>
    where
        Self: 'static + Method<P> + Sized,
        Self::Output: Future,
    {
        helpers::map::map(self, futures::executor::block_on)
    }

    #[inline(always)]
    fn wrap_with_spawn(self) -> impl Method<(), Output = ()>
    where
        Self: 'static + Method<P> + Sized,
        Self::Output: Future + Send + 'static,
    {
        self.wrap_with(|fut| move |e: &Executor| e.spawn(fut))
    }
}

impl<F, P> MethodExt<P> for F where F: Method<P> {}
