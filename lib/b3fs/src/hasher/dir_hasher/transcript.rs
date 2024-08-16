//! The directory entry binary serialization.

use arrayref::array_ref;

use super::FromTranscriptError;
use crate::entry::{BorrowedEntry, BorrowedLink};
use crate::hasher::b3::IV;
use crate::hasher::iv::IV;
use crate::utils::{is_valid_filename_len, is_valid_symlink};

pub const B3_DIR_IS_ROOT: u8 = 1 << 0;
pub const B3_DIR_IS_SYM_LINK: u8 = 1 << 1;

/// Write the transcript of a given entry at a certain position in the tree to a buffer.
///
/// The maximum size of a buffer needed in theory is:
///
/// No symbolic link: 4 + 255 + 32 = 291
/// With Symlink: 4 + 255 + 1023 + 1 = 1283
pub fn write_entry_transcript(
    buffer: &mut Vec<u8>,
    entry: BorrowedEntry,
    counter: u16,
    is_root: bool,
) {
    let (mut flag, content, clen, is_sym) = match entry.link {
        crate::entry::BorrowedLink::Content(hash) => (0, hash.as_slice(), 32, false),
        crate::entry::BorrowedLink::Path(link) => (B3_DIR_IS_SYM_LINK, link, link.len() + 1, true),
    };

    if is_root {
        flag |= B3_DIR_IS_ROOT;
    }

    buffer.reserve(4 + entry.name.len() + clen);
    buffer.push(flag);
    buffer.extend_from_slice(entry.name);

    // A valid file name never contains the "null" byte. So we can use it as terminator
    // instead of prefixing the length.
    buffer.push(0x00);

    // Fill the rest of the bytes with the content link.
    buffer.extend_from_slice(content);
    if is_sym {
        buffer.push(0x00);
    }

    // We put the counter bytes at the end which allows us to use them for
    // hashing while skipping them when writing on the disk.
    buffer.push((counter & 0xff) as u8);
    buffer.push(((counter >> 8) & 0xff) as u8);
}

/// Hash a buffer written by [`write_entry_hash`] using blake3.
#[inline]
pub fn hash_transcript(buffer: &[u8]) -> [u8; 32] {
    IV::DIR.hash_all_at_once(buffer)
}

/// Hash an entry.
pub fn hash_entry<'a, E>(entry: &'a E, counter: usize, is_root: bool) -> [u8; 32]
where
    BorrowedEntry<'a>: From<&'a E>,
{
    let mut buffer = Vec::with_capacity(320);
    write_entry_transcript(&mut buffer, entry.into(), counter as u16, is_root);
    hash_transcript(&buffer)
}

impl<'a> BorrowedEntry<'a> {
    pub fn from_transcript(transcript: &'a [u8]) -> Result<Self, FromTranscriptError> {
        let n = transcript.len();

        if n < 3 {
            return Err(FromTranscriptError::TranscriptTooSmall);
        }

        let name_sz = transcript[1..]
            .iter()
            .position(|b| *b == 0)
            .unwrap_or(usize::MAX);
        if is_valid_filename_len(name_sz) {
            return Err(FromTranscriptError::InvalidTranscript);
        }

        let name = &transcript[1..=name_sz];
        let content = &transcript[name_sz + 1..n - 2];

        let is_sym = (transcript[0] & B3_DIR_IS_SYM_LINK) != 0;

        if !is_sym && content.len() != 32 {
            return Err(FromTranscriptError::InvalidTranscript);
        }

        if is_sym && !is_valid_symlink(content) {
            return Err(FromTranscriptError::InvalidTranscript);
        }

        Ok(Self {
            name,
            link: if is_sym {
                BorrowedLink::Path(content)
            } else {
                let hash = array_ref!(content, 0, 32);
                BorrowedLink::Content(hash)
            },
        })
    }
}

#[cfg(test)]
mod tests {}
