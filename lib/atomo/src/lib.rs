//! Atomo is an atomic execution engine for Rust. At its core it is a database
//! wrapper that enhances any backend storage engine with an optimized snapshot
//! functionality.

mod batch;
mod builder;
mod db;
mod inner;
mod key_iterator;
mod keys;
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
