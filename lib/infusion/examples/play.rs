use std::marker::PhantomData;

trait A<C: Collection<A = Self>>: Sized {
    fn x(&self, o: <C::B as B>::Output);
}

trait B {
    type Collection: Collection<B = Self>;

    type Output: Iterator<Item = u8>;
}

trait Collection: 'static + Sized {
    type A: A<Self>;
    type B: B<Collection = Self>;
}

struct I<C: Collection, T>(PhantomData<T>, <C::B as B>::Output);

impl<C, T> A<C> for I<C, T>
where
    C: Collection<A = Self>,
{
    fn x(&self, o: <C::B as B>::Output) {}
}

fn main() {
    println!("BLANK");

    println!("");
}
