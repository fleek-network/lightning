use std::io::Write;

use super::error::PersistanceError;
use crate::collections::HashTree;
use crate::directory::entry::BorrowedEntry;
use crate::directory::hasher::DirectoryHasher;

/// Write a file to the given writer using the provided hashtree of the file.
pub fn write_file_with_hashtree<W>(writer: &mut W, tree: HashTree) -> Result<(), PersistanceError>
where
    W: Write,
{
    writer.write_all(&[0, 0, 0, 0])?;
    writer.write_all(tree.as_inner().as_slice())?;
    Ok(())
}

/// Write a directory using a list of provided entries.
pub fn write_directory_from_entries<'a, W, T>(
    writer: &mut W,
    entries: &'a [T],
) -> Result<[u8; 32], PersistanceError>
where
    W: Write,
    BorrowedEntry<'a>: From<&'a T>,
{
    let mut hasher = DirectoryHasher::new(true);

    writer.write_all(&[1, 0, 0, 0])?;
    writer.write_all(&u32::to_le_bytes(entries.len() as u32))?;

    let mut pos_vec = Vec::<u32>::with_capacity(entries.len());
    let mut pos = 0;

    for e in entries {
        let entry = BorrowedEntry::from(e);
        hasher.insert(entry)?;
        hasher.flush_tree(writer)?;

        pos_vec.push(pos);
        pos += compute_entry_size(entry) as u32;
    }

    let hash = hasher.finalize_flush_tree(writer)?;
    for pos in pos_vec {
        writer.write_all(&u32::to_le_bytes(pos))?;
    }

    // Now write the paylods.
    for e in entries {
        let entry = BorrowedEntry::from(e);
        write_entry(writer, entry)?;
    }

    Ok(hash)
}

#[inline(always)]
fn compute_entry_size(e: BorrowedEntry) -> usize {
    // tag (1 byte) - name (n len) - \0x00 (null byte) -
    // { [file, dir]: 32 byte, link: (len + null) }
    1 + e.name.len()
        + 1
        + match e.link {
            crate::directory::entry::BorrowedLink::File(_) => 32,
            crate::directory::entry::BorrowedLink::Directory(_) => 32,
            crate::directory::entry::BorrowedLink::Link(name) => name.len() + 1,
        }
}

#[inline(always)]
fn write_entry<W>(writer: &mut W, e: BorrowedEntry) -> std::io::Result<()>
where
    W: Write,
{
    match e.link {
        crate::directory::entry::BorrowedLink::File(hash) => {
            writer.write_all(&[0])?;
            writer.write_all(e.name)?;
            writer.write_all(&[0])?;
            writer.write_all(hash)?;
        },
        crate::directory::entry::BorrowedLink::Directory(hash) => {
            writer.write_all(&[1])?;
            writer.write_all(e.name)?;
            writer.write_all(&[0])?;
            writer.write_all(hash)?;
        },
        crate::directory::entry::BorrowedLink::Link(link) => {
            writer.write_all(&[2])?;
            writer.write_all(e.name)?;
            writer.write_all(&[0])?;
            writer.write_all(link)?;
            writer.write_all(&[0])?;
        },
    }
    Ok(())
}
