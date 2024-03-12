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
{
    type Output = U;

    #[inline(always)]
    fn display_name(&self) -> Option<String> {
        self.method.display_name()
    }

    #[inline(always)]
    fn events(&self) -> Option<crate::Eventstore> {
        self.method.events()
    }

    #[inline(always)]
    fn dependencies() -> Vec<Ty> {
        F::dependencies()
    }

    #[inline(always)]
    fn call(self, provider: &Provider) -> Self::Output {
        let output = self.method.call(provider);
        (self.transform)(output)
    }
}

#[inline(always)]
pub fn map<F, P, M, U>(f: F, transform: M) -> impl Method<P, Output = U>
where
    F: Method<P>,
    M: FnOnce(F::Output) -> U,
{
    Map {
        method: f,
        transform,
    }
}
