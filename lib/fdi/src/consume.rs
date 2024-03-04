use std::marker::PhantomData;

use crate::Method;

pub struct Consume<F, T, P, O> {
    pub(crate) f: F,
    _p: PhantomData<(F, T, P, O)>,
}

pub fn consume<F, T, P, O>(f: F) -> impl Method<F, (O, T)>
where
    Consume<F, T, P, O>: Method<F, (O, T)>,
{
    Consume { f, _p: PhantomData }
}
