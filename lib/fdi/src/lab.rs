use std::any::type_name;

use crate::provider::{Provider, ProviderGuard, Ref, RefMut};
use crate::ty::Ty;

pub trait Extractor<'e>: 'e {
    fn extract<'p: 'e>(registry: &'p ProviderGuard) -> Self;
}

impl<'e, T: 'static> Extractor<'e> for &'e T {
    fn extract<'p: 'e>(registry: &'p ProviderGuard) -> Self {
        registry.get::<T>()
    }
}

fn call_1<'a, 'p, F, T, A>(r: &'p ProviderGuard<'p>, f: F) -> T
where
    F: FnOnce(A) -> T,
    T: 'static,
    A: Extractor<'a>,
    'p: 'a,
{
    println!("{}", type_name::<T>());
    println!("{}", type_name::<A>());

    let a = A::extract(r);
    (f)(a)
}

fn call_1_outer<'a, 'p, F, T, A>(r: &'p Provider, f: F) -> T
where
    F: FnOnce(A) -> T,
    T: 'static,
    A: Extractor<'a>,
    'p: 'a,
{
    let guard = r.guard();
    // call_1(&guard, f)
    todo!()
}

#[test]
fn xxx() {
    // fn x(v: &String) {}
    // expect_method(x);
    // expect_method(|a: &String| {});
    let p = Provider::default();
    p.insert(String::from("Hello"));

    // // call_1(&p, |a: Ref<String>| a.clone());
    // let g = p.guard();
    // let out = call_1(&g, |a: &String| a.clone());

    // println!("{}", out);

    // let x = p.get_mut::<String>();
    // println!("{}", *x);
}
