//! Atomo is an atomic execution engine for Rust. At its core it is a database
//! wrapper that enhances any backend storage engine with an optimized snapshot
//! functionality.

pub mod batch;
mod builder;
mod db;
mod inner;
mod key_iterator;
mod keys;
mod serder;
mod snapshot;
pub mod storage;
mod table;

pub type DefaultSerdeBackend = serder::BincodeSerde;

pub use builder::AtomoBuilder;
pub use db::{Atomo, QueryPerm, TableId, UpdatePerm};
pub use key_iterator::KeyIterator;
pub use serder::{BincodeSerde, SerdeBackend};
pub use storage::{InMemoryStorage, StorageBackend, StorageBackendConstructor};
pub use table::{ResolvedTableReference, TableRef, TableSelector};
