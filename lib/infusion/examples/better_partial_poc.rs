// This is a proof of concept for how we can have a better `partial!` and an almost on demand
// modification to a collection.

use std::marker::PhantomData;

trait A<X: C> {
    fn visit() {
        println!("visiting {}", std::any::type_name::<Self>());
        println!("A = {}", std::any::type_name::<X::A>());
        println!("B = {}", std::any::type_name::<X::B>());
    }
}

trait B<X: C> {
    fn visit() {
        println!("visiting {}", std::any::type_name::<Self>());
        println!("A = {}", std::any::type_name::<X::A>());
        println!("B = {}", std::any::type_name::<X::B>());
    }
}

struct A1<X: C>(PhantomData<X>);
struct A2<X: C>(PhantomData<X>);
struct B1<X: C>(PhantomData<X>);
struct B2<X: C>(PhantomData<X>);

impl<X: C> A<X> for A1<X> {}
impl<X: C> A<X> for A2<X> {}
impl<X: C> B<X> for B1<X> {}
impl<X: C> B<X> for B2<X> {}

trait CBase {
    type A<X: C>: A<X>;
    type B<X: C>: B<X>;
}

trait C: Sized {
    type A: A<Self>;
    type B: B<Self>;

    fn visit() {
        println!("visiting {}", std::any::type_name::<Self>());
        println!("A = {}", std::any::type_name::<Self::A>());
        println!("B = {}", std::any::type_name::<Self::B>());

        <Self::A as A<Self>>::visit();
        <Self::B as B<Self>>::visit();
    }
}

impl<T: CBase> C for T {
    type A = T::A<Self>;
    type B = T::B<Self>;
}

struct F1;
impl CBase for F1 {
    type A<X: C> = A1<X>;
    type B<X: C> = B1<X>;
}

struct F2;
impl CBase for F2 {
    type A<X: C> = <F1 as CBase>::A<X>;
    type B<X: C> = B2<X>;
}

trait _AModifier {
    type A<X: C>: A<X>;
}
struct AModifier<N: _AModifier, O: CBase>(PhantomData<(N, O)>);
impl<N: _AModifier, O: CBase> CBase for AModifier<N, O> {
    type A<X: C> = N::A<X>;
    type B<X: C> = O::B<X>;
}

struct UseA1;
impl _AModifier for UseA1 {
    type A<X: C> = A1<X>;
}
struct UseA2;
impl _AModifier for UseA2 {
    type A<X: C> = A2<X>;
}

fn main() {
    println!("-------------------- F1");
    F1::visit();

    println!("-------------------- Modified");
    type X = AModifier<UseA2, F1>;
    X::visit();
}
