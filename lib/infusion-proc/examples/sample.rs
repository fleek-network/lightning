#[infusion_proc::input]
trait A<C: Collection, W> {}

trait Collection {}

#[infusion_proc::input]
trait B<C: Collection> {}

#[infusion_proc::input]
trait C {}

pub mod infusion {
    use std::marker::PhantomData;

    pub struct Blank<C>(PhantomData<C>);

    pub struct Container;

    pub mod graph {
        pub struct DependencyGraphVisitor;

        impl DependencyGraphVisitor {
            pub fn mark_input(&mut self) {}
        }
    }
}

fn main() {}
