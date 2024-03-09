use futures::Future;

use crate::{x_helpers, Executor, Method};

pub trait MethodExt<P>: Sized + Method<P> {
    #[inline(always)]
    fn to_infallible(self) -> impl Method<P, Output = anyhow::Result<Self::Output>> {
        x_helpers::map::map(self, Ok)
    }

    #[inline(always)]
    fn with_display_name(self, name: impl Into<String>) -> impl Method<P, Output = Self::Output> {
        x_helpers::display_name::with_display_name(self, name)
    }

    #[inline(always)]
    fn map_display_name<M>(self, map: M) -> impl Method<P, Output = Self::Output>
    where
        M: FnOnce(String) -> String,
    {
        x_helpers::display_name::map_display_name(self, map)
    }

    #[inline(always)]
    fn map<M, U>(self, transform: M) -> impl Method<P, Output = U>
    where
        M: FnOnce(Self::Output) -> U,
        U: 'static,
    {
        x_helpers::map::map(self, transform)
    }

    #[inline(always)]
    fn on<H, Y>(self, event: &'static str, handler: H) -> impl Method<P, Output = Self::Output>
    where
        H: Method<Y>,
    {
        x_helpers::on::on(self, event, handler)
    }

    #[inline(always)]
    fn wrap_with<W, OutW, ArgsOutW>(self, wrapper: W) -> impl Method<(), Output = OutW::Output>
    where
        W: FnOnce(Self::Output) -> OutW,
        OutW: Method<ArgsOutW>,
    {
        x_helpers::wrap_with::wrap_with(self, wrapper)
    }

    #[inline(always)]
    fn block_on(self) -> impl Method<P, Output = <Self::Output as Future>::Output>
    where
        Self: 'static + Method<P> + Sized,
        Self::Output: Future,
    {
        x_helpers::map::map(self, futures::executor::block_on)
    }

    #[inline(always)]
    fn spawn(self) -> impl Method<(), Output = ()>
    where
        Self: 'static + Method<P> + Sized,
        Self::Output: Future + Send,
    {
        self.wrap_with(|fut| move |e: &Executor| e.spawn(fut))
    }
}

impl<F, P> MethodExt<P> for F where F: Method<P> {}
