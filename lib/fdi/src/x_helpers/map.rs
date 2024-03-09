use crate::ty::Ty;
use crate::{Method, Provider};

struct Map<F, M> {
    method: F,
    transform: M,
}

impl<F, P, M, U> Method<P> for Map<F, M>
where
    F: Method<P>,
    M: FnOnce(F::Output) -> U,
    U: 'static,
{
    type Output = U;

    #[inline(always)]
    fn dependencies() -> Vec<Ty> {
        F::dependencies()
    }

    #[inline(always)]
    fn call(self, registry: &Provider) -> Self::Output {
        let output = self.method.call(registry);
        (self.transform)(output)
    }
}

#[inline(always)]
pub fn map<F, P, M, U>(f: F, transform: M) -> impl Method<P, Output = U>
where
    F: Method<P>,
    M: FnOnce(F::Output) -> U,
    U: 'static,
{
    Map {
        method: f,
        transform,
    }
}
