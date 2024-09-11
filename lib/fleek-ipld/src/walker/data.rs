use bytes::Bytes;
use ipld_core::cid::Cid;

use crate::decoder::fs::DocId;

#[derive(Clone, Debug)]
pub struct Dir {
    pub id: DocId,
    pub name: Option<String>,
}

#[derive(Clone, Debug)]
pub struct Chunked {
    pub index: u64,
    pub total: u64,
}

#[derive(Clone)]
pub struct ItemFile {
    pub id: DocId,
    pub name: Option<String>,
    pub size: Option<u64>,
    pub data: Bytes,
    pub chunked: Option<Chunked>,
}

impl std::fmt::Debug for ItemFile {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ItemFile")
            .field("id", &self.id)
            .field("name", &self.name)
            .field("size", &self.size)
            .field("chunked", &self.chunked)
            .field("data-length", &self.data.len())
            .finish()
    }
}

#[derive(Clone, Debug)]
pub enum Item {
    Directory(Dir),
    File(ItemFile),
    Skip,
}

impl Item {
    pub fn cid(&self) -> Option<&Cid> {
        match self {
            Item::Directory(dir) => Some(dir.id.cid()),
            Item::File(file) => Some(file.id.cid()),
            Item::Skip => None,
        }
    }

    pub fn is_cid(&self, cid: &str) -> bool {
        Cid::try_from(cid)
            .map(|cid| Some(&cid) == self.cid())
            .unwrap_or(false)
    }
}
