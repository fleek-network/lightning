use futures::future::LocalBoxFuture;
use futures::Future;

use crate::consume::Consume;
use crate::ty::Ty;
use crate::{Method, Registry};

macro_rules! impl_method {
    (
        [$($mut:ident)*],
        [$($get:ident)*]
    ) => {
        impl<F, T
            $(, $mut)*
            $(, $get)*
        > Method<T, (
            (
                $($mut,)*
            ),
            (
                $($get,)*
            )
        )> for F
        where
            F: FnOnce(
                $(&mut $mut,)*
                $(& $get,)*
            ) -> T,
            $($mut: 'static,)*
            $($get: 'static,)*
        {
            fn dependencies(&self) -> Vec<Ty> {
                vec![
                    $(Ty::of::<$mut>(),)*
                    $(Ty::of::<$get>(),)*
                ]
            }

            #[inline(always)]
            #[allow(unused)]
            fn call(self, registry: &Registry) -> T {
                (self)(
                    $(&mut registry.get_mut::<$mut>(),)*
                    $(&registry.get::<$get>(),)*
                )
            }

            #[allow(unused)]
            #[allow(non_snake_case)]
            fn async_call<U>(self, registry: &Registry) -> LocalBoxFuture<'static, U>
            where
                T: Future<Output = U>,
                Self: 'static + Sized,
            {
                $(let mut $mut = registry.get_mut::<$mut>();)*
                $(let $get = registry.get::<$get>();)*

                Box::pin(async move {
                    (self)(
                        $(&mut $mut,)*
                        $(&$get,)*
                    ).await
                })
            }
        }

        impl<F, T, O
            $(, $mut)*
            $(, $get)*
        > Method<T, (O,
            (
                (
                    $($mut,)*
                ),
                (
                    $($get,)*
                )
            )
        )> for Consume<F, T, (
            (
                $($mut,)*
            ),
            (
                $($get,)*
            )
        ), O>
        where
            F: FnOnce(
                O,
                $(&mut $mut,)*
                $(& $get,)*
            ) -> T,
            O: 'static,
            $($mut: 'static,)*
            $($get: 'static,)*
        {
            fn dependencies(&self) -> Vec<crate::ty::Ty> {
                vec![
                    Ty::of::<O>(),
                    $(Ty::of::<$mut>(),)*
                    $(Ty::of::<$get>(),)*
                ]
            }

            #[inline(always)]
            fn call(self, registry: &crate::Registry) -> T {
                (self.f)(
                    registry.take(),
                    $(&mut registry.get_mut::<$mut>(),)*
                    $(&registry.get::<$get>(),)*
                )
            }
        }

    };
}

impl_method!([], []);
impl_method!([], [C0]);
impl_method!([B0], []);
impl_method!([], [C0 C1]);
impl_method!([B0], [C0]);
impl_method!([B0 B1], []);
impl_method!([], [C0 C1 C2]);
impl_method!([B0], [C0 C1]);
impl_method!([B0 B1], [C0]);
impl_method!([B0 B1 B2], []);
impl_method!([], [C0 C1 C2 C3]);
impl_method!([B0], [C0 C1 C2]);
impl_method!([B0 B1], [C0 C1]);
impl_method!([B0 B1 B2], [C0]);
impl_method!([B0 B1 B2 B3], []);
impl_method!([], [C0 C1 C2 C3 C4]);
impl_method!([B0], [C0 C1 C2 C3]);
impl_method!([B0 B1], [C0 C1 C2]);
impl_method!([B0 B1 B2], [C0 C1]);
impl_method!([B0 B1 B2 B3], [C0]);
impl_method!([B0 B1 B2 B3 B4], []);
impl_method!([], [C0 C1 C2 C3 C4 C5]);
impl_method!([B0], [C0 C1 C2 C3 C4]);
impl_method!([B0 B1], [C0 C1 C2 C3]);
impl_method!([B0 B1 B2], [C0 C1 C2]);
impl_method!([B0 B1 B2 B3], [C0 C1]);
impl_method!([B0 B1 B2 B3 B4], [C0]);
impl_method!([B0 B1 B2 B3 B4 B5], []);
impl_method!([], [C0 C1 C2 C3 C4 C5 C6]);
impl_method!([B0], [C0 C1 C2 C3 C4 C5]);
impl_method!([B0 B1], [C0 C1 C2 C3 C4]);
impl_method!([B0 B1 B2], [C0 C1 C2 C3]);
impl_method!([B0 B1 B2 B3], [C0 C1 C2]);
impl_method!([B0 B1 B2 B3 B4], [C0 C1]);
impl_method!([B0 B1 B2 B3 B4 B5], [C0]);
impl_method!([B0 B1 B2 B3 B4 B5 B6], []);
impl_method!([], [C0 C1 C2 C3 C4 C5 C6 C7]);
impl_method!([B0], [C0 C1 C2 C3 C4 C5 C6]);
impl_method!([B0 B1], [C0 C1 C2 C3 C4 C5]);
impl_method!([B0 B1 B2], [C0 C1 C2 C3 C4]);
impl_method!([B0 B1 B2 B3], [C0 C1 C2 C3]);
impl_method!([B0 B1 B2 B3 B4], [C0 C1 C2]);
impl_method!([B0 B1 B2 B3 B4 B5], [C0 C1]);
impl_method!([B0 B1 B2 B3 B4 B5 B6], [C0]);
impl_method!([B0 B1 B2 B3 B4 B5 B6 B7], []);
