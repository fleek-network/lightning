use crate::consume::Consume;
use crate::extractor::Extractor;
use crate::ty::Ty;
use crate::{Method, Provider, Ref, RefMut};

macro_rules! impl_method {
    (
        [$($arg:ident)*]
    ) => {
        #[allow(unused)]
        impl<F, T
            $(, $arg)*
        > Method<T, (
            $($arg,)*
        )> for F
        where
            F: FnOnce(
                $($arg,)*
            ) -> T,
            $($arg: for<'a> Extractor<'a>,)*
        {
            fn dependencies(&self) -> Vec<Ty> {
                let mut out = Vec::new();
                $($arg::dependencies(&mut out);)*
                out
            }

            #[inline(always)]
            fn call(self, registry: &Provider) -> T {
                let guard = registry.guard();
                (self)(
                    $($arg::extract(&guard),)*
                )
            }
        }
    };
}

impl_method!([]);
impl_method!([A0]);
impl_method!([A0 A1]);
impl_method!([A0 A1 A2]);
impl_method!([A0 A1 A2 A3]);
impl_method!([A0 A1 A2 A3 A4]);
impl_method!([A0 A1 A2 A3 A4 A5]);
impl_method!([A0 A1 A2 A3 A4 A5 A6]);
