use smol_str::SmolStr;

use super::{hash_directory, FindEntryOutput};
use crate::utils::HashTree;
use crate::ProofBuf;

pub type Digest = [u8; 32];

#[derive(Clone, Hash, PartialEq, Eq, PartialOrd, Ord, Debug)]
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
pub struct Link(pub(crate) LinkRep);

#[derive(Clone, Hash, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub(crate) enum LinkRep {
    Symlink(SmolStr),
    File(Digest),
    Directory(Digest),
}

impl Directory {
    /// Create a new directory from a list of entries. The `sorted` parameters must be set to false
    /// if the provided list of entries is not already sorted.
    ///
    /// 1. The correct ordering for entries is ascending order based on the name of each entry.
    /// 2. The names *MUST* be unique.
    pub fn new(mut entries: Vec<DirectoryEntry>, skip_sorting: bool) -> Self {
        if !skip_sorting {
            entries.sort_unstable_by(|a, b| a.name.cmp(&b.name))
        }

        let tree = hash_directory(true, &entries).tree.unwrap();

        Self { entries, tree }
    }

    /// Returns the root hash of the directory.
    #[inline]
    pub fn root_hash(&self) -> &Digest {
        self.tree.get_root()
    }

    /// Search the entries for the given file and return the index if found. Otherwise an `Err(idx)`
    // is returned contaning where the element should have been.
    #[inline]
    pub fn find_index(&self, name: &str) -> Result<usize, usize> {
        self.entries.binary_search_by(|e| e.name.as_str().cmp(name))
    }

    /// Find an entry with the given name and return the link to it along with an existence proof,
    /// or if the entry is not found returns a non-existence proof.
    #[inline]
    pub fn find_entry(&self, name: &str) -> FindEntryOutput {
        self.generate_find_entry_output(self.find_index(name))
    }

    /// Generate a proof for the existence or non-existence of a given entry search.
    #[doc(hidden)]
    pub fn generate_find_entry_output(&self, index: Result<usize, usize>) -> FindEntryOutput {
        if self.entries.is_empty() {
            return FindEntryOutput::EmptyDirectory;
        }

        match index {
            Ok(idx) => {
                let proof = ProofBuf::new(self.tree.as_ref(), idx);
                let link = self.entries[idx].link().clone();
                FindEntryOutput::Found(idx, proof, link)
            },
            Err(0) => {
                let proof = ProofBuf::new(self.tree.as_ref(), 0);
                let entry = self.entries[0].clone();
                FindEntryOutput::NotFoundLeft(proof, entry)
            },
            Err(i) if i >= self.entries.len() => {
                let idx = self.entries.len() - 1;
                let proof = ProofBuf::new(self.tree.as_ref(), idx);
                let entry = self.entries[idx].clone();
                FindEntryOutput::NotFoundRight(idx, proof, entry)
            },
            Err(mid) => {
                let left_idx = mid - 1;
                let left_proof = ProofBuf::new(self.tree.as_ref(), left_idx);
                let left_entry = self.entries[left_idx].clone();
                let right_idx = mid + 1;
                let right_proof = ProofBuf::resume(self.tree.as_ref(), right_idx);
                let right_entry = self.entries[right_idx].clone();
                FindEntryOutput::NotFound(mid, left_proof, left_entry, right_proof, right_entry)
            },
        }
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
            LinkRep::Symlink(path) => Some(path),
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
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::directory::test_utils::*;

    #[test]
    fn directory_constructor_ordering() {
        let expected = Directory::new(vec![entry(0), entry(1), entry(2)], true);
        let actual = Directory::new(vec![entry(1), entry(2), entry(0)], false);
        assert_eq!(expected, actual);
    }

    #[test]
    fn directory_find_index() {
        let test_dir = mkdir(1..7);
        for i in 1..7 {
            // 0 3
            // i = 1
            assert_eq!(Ok(i - 1), test_dir.find_index(name(i)));
        }
        assert_eq!(Err(0), test_dir.find_index(name(0)));
        assert_eq!(Err(6), test_dir.find_index(name(7)));
        assert_eq!(Err(6), test_dir.find_index(name(8)));

        let test_dir = mkdir((1..7).filter(|e| *e != 3));
        assert_eq!(Err(2), test_dir.find_index("D"));
    }
}
