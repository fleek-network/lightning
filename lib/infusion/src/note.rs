//! # Tracking Unique Struct + Trait
//!
//! To build the dependency graph, we first need to have a way to distinguish
//! different objects at runtime. Ideally, this should happen without much
//! overhead.
//!
//! The standard library has the [TypeId](std::any::TypeId) that could be used,
//! however we do not really care about the concrete type that implements a trait.
//!
//! In the edge case that a single concrete type provides implementation for two
//! different trait, we need to be aware.
//!
//! ```ignore
//! trait A {
//!     infu!(A, {
//!         fn init(b: B) {
//!             // ...
//!         }
//!     })
//! }
//!
//! trait B {
//!     infu!(B);
//! }
//!
//! struct I;
//!
//! impl A for I { ... }
//! impl B for I { ... }
//!
//! infu!(@ [
//!     A,
//!     B
//! ]);
//!
//! struct Binding;
//!
//! impl Collection for Binding {
//!     type A = I;
//!     type B = I;
//! }
//! ```
//!
//! Use of `TypeId` would cause the creation of the link `I -> I` in the dependency graph,
//! even though the actual edge should be `<I as A> -> <I as B>`.
//!
//! There is no native API in the language for us to distinguish the two from each other,
//! but there is one workaround which is what we use in this implementation.
//!
//! Since all service traits have at least one method in common with each other, we can
//! use that function as the identifier.
//!
//! In the implementation we use the `infu_dependencies` method for this purpose, in this
//! case we can use the pointer to the function on the type.
//!
//!
//! ```ignore
//! fn demo() {
//!     let i_a_tid = (<I as A>::infu_dependencies as *const usize) as usize;
//!     let i_b_tid = (<I as B>::infu_dependencies as *const usize) as usize;
//!     assert_ne!(i_a_tid, i_b_tid);
//! }
//! ```
//!
//! This trick allows us to provide a unique identifier for the different types per trait.
//!
//! The [vtable](crate::vtable) module provides the implementation of building blocks of this
//! strategy.
//!
//! The next issue here is that `--release` can aggressively inline methods so they sometimes
//! can be the same number for different things.
