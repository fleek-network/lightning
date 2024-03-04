use crate::{helpers, Method};

pub trait MethodExt<T, P>: Sized + Method<T, P>
where
    T: 'static,
{
    fn with_display_name(self, name: &'static str) -> impl Method<T, P> {
        helpers::display_name(name, self)
    }
}

impl<F, T, P> MethodExt<T, P> for F
where
    F: Method<T, P>,
    T: 'static,
{
}
