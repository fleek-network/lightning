use crate::{helpers, Method};

pub trait MethodExt<T, P>: Sized + Method<T, P>
where
    T: 'static,
{
    #[inline(always)]
    fn with_display_name(self, name: &'static str) -> impl Method<T, P> {
        helpers::display_name(name, self)
    }

    #[inline(always)]
    fn to_infallible(self) -> impl Method<anyhow::Result<T>, P> {
        helpers::to_infalliable(self)
    }

    #[inline(always)]
    fn on<H, X, Y>(self, event: &'static str, handler: H) -> impl Method<T, P>
    where
        H: Method<X, Y>,
        X: 'static,
    {
        helpers::on(self, event, handler)
    }

    #[inline(always)]
    fn wrap_with<W, U, A, R>(self, f: W) -> impl Method<R, (P, A)>
    where
        W: Method<U, A>,
        U: 'static + FnOnce(T) -> R,
    {
        helpers::wrap(self, f)
    }
}

impl<F, T, P> MethodExt<T, P> for F
where
    F: Method<T, P>,
    T: 'static,
{
}
