use crate::Method;

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

    fn dependencies() -> Vec<crate::ty::Ty> {
        todo!()
    }

    fn call(self, registry: &crate::Provider) -> Self::Output {
        todo!()
    }
}

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
