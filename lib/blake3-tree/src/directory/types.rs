use smol_str::SmolStr;

use super::hash_directory;
use crate::utils::HashTree;

pub type Digest = [u8; 32];

pub struct Directory {
    pub entries: Vec<DirectoryEntry>,
    pub tree: HashTree,
}

#[derive(Clone, Hash, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub struct DirectoryEntry {
    name: SmolStr,
    link: Link,
}

#[derive(Clone, Hash, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub struct Link(LinkRep);

#[derive(Clone, Hash, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub enum LinkRep {
    Symlink(SmolStr),
    File(Digest),
    Directory(Digest),
}

impl Directory {
    /// Create a new directory from a list of entries. The `sorted` parameters must be set to false
    /// if the provided list of entries is not already sorted.
    pub fn new(mut entries: Vec<DirectoryEntry>, sorted: bool) -> Self {
        if !sorted {
            entries.sort_unstable_by(|a, b| a.name.cmp(&b.name))
        }

        let tree = hash_directory(true, &entries).tree.unwrap();

        Self { entries, tree }
    }
}

impl Link {
    pub fn symlink(path: impl Into<SmolStr>) -> Self {
        Self(LinkRep::Symlink(path.into()))
    }

    pub fn file(digest: Digest) -> Self {
        Self(LinkRep::File(digest))
    }

    pub fn directory(digest: Digest) -> Self {
        Self(LinkRep::Directory(digest))
    }

    pub fn is_symlink(&self) -> bool {
        matches!(&self.0, LinkRep::Symlink(_))
    }

    pub fn is_file(&self) -> bool {
        matches!(&self.0, LinkRep::File(_))
    }

    pub fn is_dir(&self) -> bool {
        matches!(&self.0, LinkRep::Directory(_))
    }

    /// Returns the digest of the file or directory this link is pointing to or `None`
    /// if this is a symlink.
    pub fn target(&self) -> Option<&[u8; 32]> {
        match &self.0 {
            LinkRep::File(digest) | LinkRep::Directory(digest) => Some(digest),
            _ => None,
        }
    }

    pub fn symlink_target(&self) -> Option<&str> {
        match &self.0 {
            LinkRep::Symlink(path) => Some(&path),
            _ => None,
        }
    }
}

impl DirectoryEntry {
    pub const fn new(name: SmolStr, link: Link) -> Self {
        Self { name, link }
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn link(&self) -> &Link {
        &self.link
    }

    pub fn to_name(self) -> SmolStr {
        self.name
    }

    pub fn to_link(self) -> Link {
        self.link
    }

    /// Returns a length-prefixed encoding of this directory entry which can be used
    /// for hashing the entry.
    #[inline(always)]
    pub(crate) fn transcript(&self, out: &mut Vec<u8>, counter: usize, is_root: bool) {
        let name_bytes = self.name.as_bytes();
        let name_len: [u8; 4] = (name_bytes.len() as u32).to_le_bytes();
        let counter: [u8; 4] = (counter as u32).to_le_bytes();

        let mut size = 1 + 4 + 4 + name_bytes.len();
        size += match &self.link.0 {
            LinkRep::Symlink(path) => 5 + path.len(),
            LinkRep::File(_) => 33,
            LinkRep::Directory(_) => 33,
        };

        out.reserve(size);
        out.push(if is_root { 1 } else { 0 });
        out.extend_from_slice(counter.as_slice());
        out.extend_from_slice(name_len.as_slice());
        out.extend_from_slice(name_bytes);
        match &self.link.0 {
            LinkRep::Symlink(path) => {
                let bytes = path.as_bytes();
                let len: [u8; 4] = (bytes.len() as u32).to_le_bytes();
                out.push(0);
                out.extend_from_slice(len.as_slice());
                out.extend_from_slice(bytes);
            },
            LinkRep::File(digest) => {
                out.push(1);
                out.extend_from_slice(digest);
            },
            LinkRep::Directory(digest) => {
                out.push(2);
                out.extend_from_slice(digest);
            },
        }
    }
}
