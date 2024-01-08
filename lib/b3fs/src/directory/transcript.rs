use super::constants::*;
use super::entry::BorrowedEntry;

/// Write the transcript of a given entry at a certain position in the tree to a buffer.
///
/// The maximum size of a buffer needed in theory is:
///
/// No symbolic link: 5 + 255 + 32 = 292
/// With Symlink: 5 + 255 + 1023 = 1283
pub fn write_entry_transcript(
    buffer: &mut Vec<u8>,
    entry: BorrowedEntry,
    counter: u16,
    is_root: bool,
) {
    let (mut flag, content) = match entry.link {
        super::entry::BorrowedLink::File(hash) => (IS_FILE_FLAG, hash.as_slice()),
        super::entry::BorrowedLink::Directory(hash) => (IS_DIR_FLAG, hash.as_slice()),
        super::entry::BorrowedLink::Link(link) => (IS_SYM_FLAG, link),
    };

    if is_root {
        flag |= IS_ROOT_FLAG;
    }

    buffer.reserve(5 + entry.name.len() + content.len());
    buffer.push(flag);
    buffer.push((counter & 0xff) as u8);
    buffer.push(((counter >> 8) & 0xff) as u8);
    buffer.extend_from_slice(entry.name);
    // A valid file name never contains the "null" byte. So we can use it as terminator
    // instead of prefixing the length.
    buffer.push(0x00);
    buffer.extend_from_slice(content);
    // Same note as above goes for content of a symbolic link.
    buffer.push(0x00);
}

/// Hash a buffer written by [`write_entry_hash`] using blake3.
#[inline]
pub fn hash_transcript(buffer: &[u8]) -> [u8; 32] {
    *fleek_blake3::keyed_hash(&KEY, &buffer).as_bytes()
}
