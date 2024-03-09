use crate::extractor::Extractor;
use crate::{Provider, Ref};

trait M<Args> {
    type Output;
    fn call(self, provider: &Provider) -> Self::Output;
}

// impl<F, T, A0> M<(A0,)> for F
// where
//     A0: 'static,
//     for<'a> F: 'a + FnOnce(A0) -> T,
//     for<'a> A0: 'a + Extractor<'a>,
//     T: 'static,
// {
//     type Output = T;

//     fn call(self, provider: &Provider) -> Self::Output {
//         let guard = provider.guard();
//         (self)(guard.extract())
//     }
// }

impl<F, T, A0> M<((), (), (A0,))> for F
where
    F: FnOnce(&A0) -> T,
    T: 'static,
    A0: 'static,
{
    type Output = T;
    fn call(self, provider: &Provider) -> T {
        let guard = provider.guard();
        (self)(guard.extract())
    }
}

impl<F, T, A0> M<((A0,), (), ())> for F
where
    F: FnOnce(A0) -> T,
    T: 'static,
    A0: 'static + for<'x> Extractor<'x>,
{
    type Output = T;
    fn call(self, provider: &Provider) -> T {
        let guard = provider.guard();
        (self)(guard.extract())
    }
}

// impl<F, T, A0, A1> M<(A0, A1)> for F
// where
//     F: FnOnce(A0, A1) -> T,
//     T: 'static,
//     A0: for<'a> Extractor<'a>,
//     A1: for<'a> Extractor<'a>,
// {
//     type Output = T;
//     fn call(self, provider: &Provider) -> Self::Output {
//         let guard = provider.guard();
//         (self)(guard.extract(), guard.extract())
//     }
// }

fn expect_method<F, Args>(f: F)
where
    F: M<Args>,
{
}

fn demo() {
    expect_method(|s: &String| {});
    expect_method(|s: Ref<String>| {});
}

#[test]
fn gen() {
    const N: usize = 7;

    for take in 0..=1 {
        let o0 = (0..take)
            .map(|a| format!("T{a}"))
            .collect::<Vec<_>>()
            .join(" ");
        let remaining = N - take;
        for mut_ref in 0..=remaining {
            let o1 = (0..mut_ref)
                .map(|a| format!("M{a}"))
                .collect::<Vec<_>>()
                .join(" ");
            let remaining = N - take - mut_ref;
            for imut_ref in 0..=remaining {
                let o2 = (0..imut_ref)
                    .map(|a| format!("R{a}"))
                    .collect::<Vec<_>>()
                    .join(" ");
                let remaining = N - take - mut_ref - imut_ref;
                for ext in 0..=remaining {
                    let o3 = (0..ext)
                        .map(|a| format!("E{a}"))
                        .collect::<Vec<_>>()
                        .join(" ");
                    println!("impl_method!([{o0}], [{o1}], [{o2}], [{o3}]);");
                }
            }
        }
    }
}
