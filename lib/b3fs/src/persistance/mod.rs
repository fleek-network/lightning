//! To deal with the on-disk format for the blockstore and to read and write content to it.

pub mod dir_reader;
pub mod error;
pub mod reader;
pub mod writer;

pub use dir_reader::{DirectoryReader, EntriesIter};
pub use error::PersistanceError;
pub use reader::Reader;
