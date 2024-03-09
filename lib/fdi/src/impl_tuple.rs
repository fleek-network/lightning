use crate::extractor::Extractor;
use crate::ty::Ty;
use crate::{Method, Provider};

macro_rules! impl_method {
    (
        [$($mut:ident)*],
        [$($get:ident)*],
        [$($ext:ident)*]
    ) => {
        #[allow(clippy::all, unused, unused_mut)]
        impl<F, T
            $(, $mut)*
            $(, $get)*
            $(, $ext)*
        > Method<(
            (
                $($mut,)*
            ),
            (
                $($get,)*
            ),
            (
                $($ext,)*
            ),
        )> for F
        where
            F: 'static + FnOnce(
                $(&mut $mut,)*
                $(& $get,)*
                $($ext,)*
            ) -> T,
            T: 'static,
            $($mut: 'static,)*
            $($get: 'static,)*
            $($ext: 'static + for<'x> Extractor<'x>,)*
        {
            type Output = T;

            fn dependencies(&self) -> Vec<Ty> {
                let mut out = Vec::new();
                $(out.push(Ty::of::<$mut>());)*
                $(out.push(Ty::of::<$get>());)*
                $($ext::dependencies(&mut out);)*
                out
            }

            #[inline(always)]
            fn call(self, provider: &Provider) -> T {
                let guard = provider.guard();
                (self)(
                    $(guard.extract::<&mut $mut>(),)*
                    $(guard.extract::<&$get>(),)*
                    $(guard.extract::<$ext>(),)*
                )
            }
        }
    };
}

impl_method!([], [], []);
impl_method!([], [], [E0]);
impl_method!([], [], [E0 E1]);
impl_method!([], [], [E0 E1 E2]);
impl_method!([], [], [E0 E1 E2 E3]);
impl_method!([], [], [E0 E1 E2 E3 E4]);
impl_method!([], [], [E0 E1 E2 E3 E4 E5]);
impl_method!([], [], [E0 E1 E2 E3 E4 E5 E6]);
impl_method!([], [R0], []);
impl_method!([], [R0], [E0]);
impl_method!([], [R0], [E0 E1]);
impl_method!([], [R0], [E0 E1 E2]);
impl_method!([], [R0], [E0 E1 E2 E3]);
impl_method!([], [R0], [E0 E1 E2 E3 E4]);
impl_method!([], [R0], [E0 E1 E2 E3 E4 E5]);
impl_method!([], [R0 R1], []);
impl_method!([], [R0 R1], [E0]);
impl_method!([], [R0 R1], [E0 E1]);
impl_method!([], [R0 R1], [E0 E1 E2]);
impl_method!([], [R0 R1], [E0 E1 E2 E3]);
impl_method!([], [R0 R1], [E0 E1 E2 E3 E4]);
impl_method!([], [R0 R1 R2], []);
impl_method!([], [R0 R1 R2], [E0]);
impl_method!([], [R0 R1 R2], [E0 E1]);
impl_method!([], [R0 R1 R2], [E0 E1 E2]);
impl_method!([], [R0 R1 R2], [E0 E1 E2 E3]);
impl_method!([], [R0 R1 R2 R3], []);
impl_method!([], [R0 R1 R2 R3], [E0]);
impl_method!([], [R0 R1 R2 R3], [E0 E1]);
impl_method!([], [R0 R1 R2 R3], [E0 E1 E2]);
impl_method!([], [R0 R1 R2 R3 R4], []);
impl_method!([], [R0 R1 R2 R3 R4], [E0]);
impl_method!([], [R0 R1 R2 R3 R4], [E0 E1]);
impl_method!([], [R0 R1 R2 R3 R4 R5], []);
impl_method!([], [R0 R1 R2 R3 R4 R5], [E0]);
impl_method!([], [R0 R1 R2 R3 R4 R5 R6], []);
impl_method!([M0], [], []);
impl_method!([M0], [], [E0]);
impl_method!([M0], [], [E0 E1]);
impl_method!([M0], [], [E0 E1 E2]);
impl_method!([M0], [], [E0 E1 E2 E3]);
impl_method!([M0], [], [E0 E1 E2 E3 E4]);
impl_method!([M0], [], [E0 E1 E2 E3 E4 E5]);
impl_method!([M0], [R0], []);
impl_method!([M0], [R0], [E0]);
impl_method!([M0], [R0], [E0 E1]);
impl_method!([M0], [R0], [E0 E1 E2]);
impl_method!([M0], [R0], [E0 E1 E2 E3]);
impl_method!([M0], [R0], [E0 E1 E2 E3 E4]);
impl_method!([M0], [R0 R1], []);
impl_method!([M0], [R0 R1], [E0]);
impl_method!([M0], [R0 R1], [E0 E1]);
impl_method!([M0], [R0 R1], [E0 E1 E2]);
impl_method!([M0], [R0 R1], [E0 E1 E2 E3]);
impl_method!([M0], [R0 R1 R2], []);
impl_method!([M0], [R0 R1 R2], [E0]);
impl_method!([M0], [R0 R1 R2], [E0 E1]);
impl_method!([M0], [R0 R1 R2], [E0 E1 E2]);
impl_method!([M0], [R0 R1 R2 R3], []);
impl_method!([M0], [R0 R1 R2 R3], [E0]);
impl_method!([M0], [R0 R1 R2 R3], [E0 E1]);
impl_method!([M0], [R0 R1 R2 R3 R4], []);
impl_method!([M0], [R0 R1 R2 R3 R4], [E0]);
impl_method!([M0], [R0 R1 R2 R3 R4 R5], []);
impl_method!([M0 M1], [], []);
impl_method!([M0 M1], [], [E0]);
impl_method!([M0 M1], [], [E0 E1]);
impl_method!([M0 M1], [], [E0 E1 E2]);
impl_method!([M0 M1], [], [E0 E1 E2 E3]);
impl_method!([M0 M1], [], [E0 E1 E2 E3 E4]);
impl_method!([M0 M1], [R0], []);
impl_method!([M0 M1], [R0], [E0]);
impl_method!([M0 M1], [R0], [E0 E1]);
impl_method!([M0 M1], [R0], [E0 E1 E2]);
impl_method!([M0 M1], [R0], [E0 E1 E2 E3]);
impl_method!([M0 M1], [R0 R1], []);
impl_method!([M0 M1], [R0 R1], [E0]);
impl_method!([M0 M1], [R0 R1], [E0 E1]);
impl_method!([M0 M1], [R0 R1], [E0 E1 E2]);
impl_method!([M0 M1], [R0 R1 R2], []);
impl_method!([M0 M1], [R0 R1 R2], [E0]);
impl_method!([M0 M1], [R0 R1 R2], [E0 E1]);
impl_method!([M0 M1], [R0 R1 R2 R3], []);
impl_method!([M0 M1], [R0 R1 R2 R3], [E0]);
impl_method!([M0 M1], [R0 R1 R2 R3 R4], []);
impl_method!([M0 M1 M2], [], []);
impl_method!([M0 M1 M2], [], [E0]);
impl_method!([M0 M1 M2], [], [E0 E1]);
impl_method!([M0 M1 M2], [], [E0 E1 E2]);
impl_method!([M0 M1 M2], [], [E0 E1 E2 E3]);
impl_method!([M0 M1 M2], [R0], []);
impl_method!([M0 M1 M2], [R0], [E0]);
impl_method!([M0 M1 M2], [R0], [E0 E1]);
impl_method!([M0 M1 M2], [R0], [E0 E1 E2]);
impl_method!([M0 M1 M2], [R0 R1], []);
impl_method!([M0 M1 M2], [R0 R1], [E0]);
impl_method!([M0 M1 M2], [R0 R1], [E0 E1]);
impl_method!([M0 M1 M2], [R0 R1 R2], []);
impl_method!([M0 M1 M2], [R0 R1 R2], [E0]);
impl_method!([M0 M1 M2], [R0 R1 R2 R3], []);
impl_method!([M0 M1 M2 M3], [], []);
impl_method!([M0 M1 M2 M3], [], [E0]);
impl_method!([M0 M1 M2 M3], [], [E0 E1]);
impl_method!([M0 M1 M2 M3], [], [E0 E1 E2]);
impl_method!([M0 M1 M2 M3], [R0], []);
impl_method!([M0 M1 M2 M3], [R0], [E0]);
impl_method!([M0 M1 M2 M3], [R0], [E0 E1]);
impl_method!([M0 M1 M2 M3], [R0 R1], []);
impl_method!([M0 M1 M2 M3], [R0 R1], [E0]);
impl_method!([M0 M1 M2 M3], [R0 R1 R2], []);
impl_method!([M0 M1 M2 M3 M4], [], []);
impl_method!([M0 M1 M2 M3 M4], [], [E0]);
impl_method!([M0 M1 M2 M3 M4], [], [E0 E1]);
impl_method!([M0 M1 M2 M3 M4], [R0], []);
impl_method!([M0 M1 M2 M3 M4], [R0], [E0]);
impl_method!([M0 M1 M2 M3 M4], [R0 R1], []);
impl_method!([M0 M1 M2 M3 M4 M5], [], []);
impl_method!([M0 M1 M2 M3 M4 M5], [], [E0]);
impl_method!([M0 M1 M2 M3 M4 M5], [R0], []);
impl_method!([M0 M1 M2 M3 M4 M5 M6], [], []);
