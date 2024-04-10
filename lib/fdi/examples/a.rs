// // collection!({
// //  A,
// //  B
// // })

// use std::marker::PhantomData;

// trait TraitA<C: Collection> {
//     fn hello() {
//         println!("HellO!")
//     }
// }
// trait TraitB<C: Collection> {}

// struct MockA<C>(PhantomData<C>);
// struct MockB<C>(PhantomData<C>);
// struct RealA<C>(PhantomData<C>);
// struct RealB<C>(PhantomData<C>);
// impl<C: Collection> TraitA<C> for MockA<C> {}
// impl<C: Collection> TraitB<C> for MockB<C> {}
// impl<C: Collection> TraitA<C> for RealA<C> {}
// impl<C: Collection> TraitB<C> for RealB<C> {}

// //
// struct MarkerA;
// struct MarkerB;

// trait With<Marker> {
//     type Type<C: Collection>;
// }

// trait Collection: Sized + With<MarkerA> + With<MarkerB>
// where
//     <Self as With<MarkerA>>::Type<Self>: TraitA<Self>,
//     <Self as With<MarkerB>>::Type<Self>: TraitB<Self>,
// {
//     type A: TraitA<Self>;
//     type B: TraitB<Self>;
// }

// //
// struct Base;

// fn expect_collection<C: Collection>() {
//     C::A::hello();
// }

fn main() {
    // expect_collection::<Base>();
}
