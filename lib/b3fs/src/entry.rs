//! Provides the owned and borrowed layout for a directory entry.

use smallvec::SmallVec;

/// A vec that can keep up to 24 items without allocating heap space. This is 32-bytes
/// while a normal Vec takes up only 24 bytes.
pub type InlineVec = SmallVec<[u8; 24]>;

#[derive(Clone, Debug)]
pub struct OwnedEntry {
    pub name: InlineVec,
    pub link: OwnedLink,
}

#[derive(Clone, Debug)]
pub enum OwnedLink {
    Content([u8; 32]),
    Link(InlineVec),
}

#[derive(Clone, Copy, Debug)]
pub struct BorrowedEntry<'a> {
    pub name: &'a [u8],
    pub link: BorrowedLink<'a>,
}

#[derive(Clone, Copy, Debug)]
pub enum BorrowedLink<'a> {
    Content(&'a [u8; 32]),
    Path(&'a [u8]),
}

impl<'a> From<&'a OwnedEntry> for BorrowedEntry<'a> {
    #[inline(always)]
    fn from(value: &'a OwnedEntry) -> Self {
        Self {
            name: &value.name,
            link: BorrowedLink::from(&value.link),
        }
    }
}

impl<'a> From<&'a OwnedLink> for BorrowedLink<'a> {
    #[inline(always)]
    fn from(value: &'a OwnedLink) -> Self {
        match value {
            OwnedLink::Content(hash) => BorrowedLink::Content(hash),
            OwnedLink::Link(link) => BorrowedLink::Path(link),
        }
    }
}

impl<'a> From<BorrowedEntry<'a>> for OwnedEntry {
    #[inline(always)]
    fn from(value: BorrowedEntry<'a>) -> Self {
        Self {
            name: value.name.into(),
            link: value.link.into(),
        }
    }
}

impl<'a> From<BorrowedLink<'a>> for OwnedLink {
    #[inline(always)]
    fn from(value: BorrowedLink<'a>) -> Self {
        match value {
            BorrowedLink::Content(hash) => OwnedLink::Content(*hash),
            BorrowedLink::Path(link) => OwnedLink::Link(link.into()),
        }
    }
}
