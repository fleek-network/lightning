use crate::ty::Ty;
use crate::{Method, Provider};

struct WithDisplayName<F> {
    display_name: &'static str,
    method: F,
}

impl<F, P> Method<P> for WithDisplayName<F>
where
    F: Method<P>,
{
    type Output = F::Output;

    #[inline(always)]
    fn display_name(&self) -> &'static str {
        self.display_name
    }

    #[inline(always)]
    fn dependencies() -> Vec<Ty> {
        F::dependencies()
    }

    #[inline(always)]
    fn call(self, registry: &Provider) -> Self::Output {
        self.method.call(registry)
    }
}

#[inline(always)]
pub fn with_display_name<F, P>(
    f: F,
    display_name: &'static str,
) -> impl Method<P, Output = F::Output>
where
    F: Method<P>,
{
    WithDisplayName {
        display_name,
        method: f,
    }
}

// TODO(qti3e): Display name to String
// pub fn map_display_name<F, P, N>(f: F, map: N) -> impl Method<P, Output = F::Output>
// where
//     F: Method<P>,
//     N: FnOnce(String) -> String,
// {
//     let naw_name = (map)(f.display_name());
//     WithDisplayName {
//         display_name,
//         method: f,
//     }
// }
