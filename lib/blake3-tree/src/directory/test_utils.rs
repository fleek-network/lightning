use super::{Directory, DirectoryEntry, Link};

/// Returns the `i`-th ascii character starting from `A`.
pub fn name(i: usize) -> &'static str {
    const CHARS: &str = "ABCDEFGHIJKLMNOPQRSTUVWXYZ";
    const N: usize = CHARS.len();
    let offset = i % N;
    let len = i.div_ceil(N);
    &CHARS[offset..][..len]
}

pub fn link(i: usize) -> Link {
    let mut s = [0; 32];
    s[0..8].copy_from_slice(&(i as u64).to_le_bytes());
    Link::file(s)
}

/// Returns a directory entry that is constructed from a name generated by [`name`] and a
/// link generated by [`link`]
pub fn entry(i: usize) -> DirectoryEntry {
    DirectoryEntry::new(name(i).to_string().into(), link(i))
}

/// Create a directory from the given iterator.
pub fn mkdir(iter: impl IntoIterator<Item = usize>) -> Directory {
    Directory::new(iter.into_iter().map(entry).collect(), false)
}
