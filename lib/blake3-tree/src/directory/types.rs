use smol_str::SmolStr;

pub type Digest = [u8; 32];

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
    
    pub fn to_link(&self) -> Link {
        self.link
    }

    /// Returns a length-prefixed encoding of this directory entry which can be used
    /// for hashing the entry.
    #[doc(hidden)]
    pub fn transcript(&self) -> Vec<u8> {
        let name_bytes = self.name.as_bytes();
        let name_len: [u8; 4] = (name_bytes.len() as u32).to_le_bytes();

        let mut size = 4 + name_bytes.len();
        size += match &self.link.0 {
            LinkRep::Symlink(path) => 5 + path.len(),
            LinkRep::File(_) => 33,
            LinkRep::Directory(_) => 33,
        };

        let mut vec = Vec::with_capacity(size);
        vec.extend_from_slice(name_len.as_slice());
        vec.extend_from_slice(name_bytes);
        match &self.link.0 {
            LinkRep::Symlink(path) => {
                let bytes = path.as_bytes();
                let len: [u8; 4] = (bytes.len() as u32).to_le_bytes();
                vec.push(0);
                vec.extend_from_slice(len.as_slice());
                vec.extend_from_slice(bytes);
            },
            LinkRep::File(digest) => {
                vec.push(1);
                vec.extend_from_slice(digest);
            },
            LinkRep::Directory(digest) => {
                vec.push(2);
                vec.extend_from_slice(digest);
            },
        }
        vec
    }

    /// Return the hash of the directory entry. Note that this is not the hash of the content that
    /// this entry is linking to, but is simply a compression over informations stored in this
    /// entry.
    pub fn digest(&self) -> fleek_blake3::Hash {
        let transcript = self.transcript();
        super::hash::hash_directory_entry_transcript(&transcript)
    }
}
