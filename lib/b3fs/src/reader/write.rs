use crate::directory::{entry::BorrowedEntry, hasher::DirectoryHasher};
use thiserror::Error;
use std::io::Write;
use crate::directory::error::Error as HasherError;

#[derive(Debug, Error)]
#[error(transparent)]
pub enum EncodeDirectoryError {
    /// The error happened with the hasher while trying to hash the data.
    Hasher(HasherError),
    /// The error originated from writing the bytes to the target buffer.
    Writer(std::io::Error)
}

pub fn write_directory<'a, W, T>(writer: &mut W, entries: &'a [T]) -> Result<(), EncodeDirectoryError> where W: Write, &'a T: Into<BorrowedEntry<'a>> {
    let mut hashtree = DirectoryHasher::new(true);

    // Is dir flag.
    writer.write_all(&[1, 0, 0, 0]).map_err(EncodeDirectoryError::Writer)?;
    writer.write_all(&entries.len().to_le_bytes()).map_err(EncodeDirectoryError::Writer)?;

    for entry in entries {
        let e : BorrowedEntry = entry.into();
        hashtree.insert(e).map_err(EncodeDirectoryError::Hasher)?;
        hashtree.flush_tree()
    }

    todo!()
}
