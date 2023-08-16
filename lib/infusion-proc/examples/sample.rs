use std::convert::Infallible;

#[infusion_proc::service]
trait A<C: Collection> {
    const X: u8 = 121;

    fn _init(a: ::A, b: ::B) {
        let x = b.xxx();
        Result::<Self, Infallible>::Ok(todo!())
    }

    fn p(b: &<C as Collection>::B) {
        let x = b.xxx();
    }

    fn t(&self);
}

#[infusion_proc::service]
trait B<C: Collection> {
    fn xxx(&self) {}

    fn _post(&mut self) {
        self.xxx();
    }

    fn x(&mut self) {
        let _x = self.xxx();
    }
}

trait Collection: Sized + 'static {
    type A: A<Self> + 'static;
    type B: B<Self> + 'static;
}

struct C {}
impl Collection for C {
    type A = infusion::Blank<Self>;
    type B = infusion::Blank<Self>;
}


fn main() {
    let x = <infusion::Blank<C> as A<C>>::X;
    println!("x = {x}");
}
