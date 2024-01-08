//! A zero-copy implementation of the on-disk format for file and directory header files.

pub mod directory;
pub mod reader;

pub use directory::DirectoryReader;
pub use reader::Reader;
