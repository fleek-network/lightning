use std::cmp::Ordering;

use crate::collections::HashTree;
use crate::directory::entry::{BorrowedEntry, BorrowedLink};

/// A zero-copy deserializer for a directory.
pub struct DirectoryReader<'b> {
    buffer: &'b [u8],
}

impl<'b> DirectoryReader<'b> {
    /// Creates a new directory from the provided buffer. You should know that this method does
    /// not perform any checks on the validity of the provided buffer and this can cause errors
    /// and possible panics when methods are called in future.
    ///
    /// Another important thing to know is that this buffer is different than the buffer of the
    /// header given that it does not contain the tag byte.
    pub fn new(buffer: &'b [u8]) -> Self {
        Self { buffer }
    }

    /// Returns the number of entries in this directory.
    pub fn len(&self) -> usize {
        read_u32(self.buffer, 0) as usize
    }

    /// Returns the hash tree of this directory.
    pub fn get_tree(&self) -> HashTree<'_> {
        let e = self.after_tree_pos();
        HashTree::try_from(&self.buffer[4..e]).expect("Could not read the hash tree.")
    }

    /// Search for the given entry with the provided name in the directory and return
    /// the name for it.
    pub fn search(&self, name: &[u8]) -> Option<(usize, BorrowedEntry<'_>)> {
        let len = self.len();
        if len == 0 {
            return None;
        }

        let offset = self.after_tree_pos();
        let slice = &self.buffer[offset..];

        let mut hi = len - 1;
        let mut lo = 0;

        while lo <= hi {
            let mid = (hi + lo) / 2;
            let offset = read_u32(slice, mid << 2) as usize;

            // +1 because the first byte of the entry bytes is the tag.
            match cmp(name, &slice[offset + 1..]) {
                Ordering::Equal => {
                    return Some((mid, read_entry(&slice[offset..])));
                },
                Ordering::Less => {
                    hi = mid - 1;
                },
                Ordering::Greater => {
                    lo = mid + 1;
                },
            }
        }

        None
    }

    #[inline(always)]
    pub fn after_tree_pos(&self) -> usize {
        4 + ((2 * self.len()).max(1) - 1) << 5
    }
}

/// Read a little endian [`u32`] from the given offset.
#[inline(always)]
fn read_u32(buffer: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes(*arrayref::array_ref![buffer, offset, 4])
}

/// Returns the result of comparing the target with the provided null terminated buffer.
#[inline]
fn cmp(target: &[u8], buffer: &[u8]) -> Ordering {
    let mut i = 0;

    loop {
        let is_target_over = i == target.len();
        let is_buffer_over = i == buffer.len() || buffer[i] == 0;

        match (is_target_over, is_buffer_over) {
            (true, true) => return Ordering::Equal,
            (true, false) => return Ordering::Less,
            (false, true) => return Ordering::Greater,
            (false, false) => {},
        }

        match target[i].cmp(&buffer[i]) {
            Ordering::Equal => {
                i += 1;
            },
            Ordering::Less => {
                return Ordering::Less;
            },
            Ordering::Greater => {
                return Ordering::Greater;
            },
        }
    }
}

#[inline(always)]
fn read_entry<'a>(buffer: &'a [u8]) -> BorrowedEntry<'a> {
    let name = read_null_terminated(&buffer[1..]);
    let lnk_offset = 2 + name.len();
    let link = match buffer[0] {
        0 => BorrowedLink::File(arrayref::array_ref![buffer, lnk_offset, 32]),
        1 => BorrowedLink::Directory(arrayref::array_ref![buffer, lnk_offset, 32]),
        2 => BorrowedLink::Link(read_null_terminated(&buffer[lnk_offset..])),
        _ => unreachable!(),
    };
    BorrowedEntry { name, link }
}

#[inline(always)]
fn read_null_terminated<'a>(buffer: &'a [u8]) -> &'a [u8] {
    for (i, b) in buffer.iter().enumerate() {
        if *b == 0 {
            return &buffer[..i];
        }
    }
    buffer
}
