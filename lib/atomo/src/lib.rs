mod batch;
mod builder;
mod db;
mod inner;
mod key_iterator;
mod once_ptr;
mod serder;
mod snapshot;
mod table;

pub type DefaultSerdeBackend = serder::BincodeSerde;

pub use builder::AtomoBuilder;
pub use db::{Atomo, QueryPerm, UpdatePerm};
pub use key_iterator::KeyIterator;
// TODO(qti3e): Maybe move to its own crate?
pub use once_ptr::OncePtr;
pub use serder::{BincodeSerde, SerdeBackend};
pub use table::{ResolvedTableReference, TableRef, TableSelector};

#[deprecated = "`MtAtomo` is deprecated. Use `Atomo` instead."]
pub type MtAtomo<O, S> = Atomo<O, S>;

#[deprecated = "`MtAtomoBuilder` is deprecated. Use `AtomoBuilder` instead."]
pub type MtAtomoBuilder<S> = AtomoBuilder<S>;
