use futures::future::LocalBoxFuture;
use futures::Future;

use crate::Registry;

trait Method<'r, T, P> {
    fn call(self, r: &'r Registry) -> T;
}

impl<'a, 'r: 'a, F, T, A0> Method<'r, T, ((), (A0,))> for F
where
    F: 'r + FnOnce(&'a A0) -> T,
    A0: 'static,
{
    fn call(self, r: &'r Registry) -> T {
        let a = r.get::<A0>();
        let a = &*a;
        let out = (self)(a);

        todo!()
    }
}

fn expect_async<'a, F, T, P, U>(f: F)
where
    F: Method<'a, T, P>,
    T: Future<Output = U>,
{
}

fn is_method<'a, F, T, P>(f: F)
where
    F: Method<'a, T, P>,
{
}

#[test]
fn async_demo() {
    let r = Registry::default();

    struct A;
    impl A {
        async fn work(&self) {}
    }

    is_method(A::work);

    expect_async(|r: &Registry| {
        let r = r.snapshot();
        Box::pin(async move {
            let a = r.get::<A>();
            a.work().await;
        })
    });
}

fn to_boxed_future_fn<'a, F, A, Fut, O>(f: F) -> impl FnOnce(A) -> LocalBoxFuture<'a, O>
where
    F: 'static + Sized + FnOnce(A) -> Fut,
    Fut: Future<Output = O>,
    A: 'a,
{
    |a| Box::pin(async move { f(a).await })
}
