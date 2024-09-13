use std::path::PathBuf;

use bytes::Bytes;
use ipld_core::cid::Cid;
use typed_builder::TypedBuilder;

use crate::decoder::fs::{DocId, IpldItem, Link};

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

impl ItemFile {
    pub fn is_chunked(&self) -> bool {
        self.chunked.is_some()
    }
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

    pub(crate) fn from_ipld(
        item: &IpldItem,
        Metadata {
            size,
            name,
            index,
            total,
            ..
        }: Metadata,
    ) -> Item {
        match item {
            IpldItem::Dir(dir) => Item::Directory(Dir {
                id: dir.id().clone(),
                name,
            }),
            IpldItem::File(file) => Item::File(ItemFile {
                id: file.id().clone(),
                name,
                size,
                data: file.data().clone(),
                chunked: index.and_then(|index| total.map(|total| Chunked { index, total })),
            }),
            IpldItem::ChunkedFile(_) => Item::Skip,
            IpldItem::Chunk(_) => Item::Skip,
        }
    }
}

/// This is an internal struct used to pass metadata to the `Item` constructor.
#[derive(Default, Debug, Clone, TypedBuilder)]
pub(crate) struct Metadata {
    #[builder(default = PathBuf::new())]
    parent_path: PathBuf,
    #[builder(default)]
    size: Option<u64>,
    #[builder(default)]
    name: Option<String>,
    #[builder(default, setter(into, strip_option))]
    index: Option<u64>,
    #[builder(default, setter(into, strip_option))]
    total: Option<u64>,
}

impl Metadata {
    pub(crate) fn new(index: usize, total: usize, link: &Link, parent_path: PathBuf) -> Self {
        Metadata::builder()
            .parent_path(parent_path)
            .size(*link.size())
            .name(link.name().clone())
            .index(index as u64)
            .total(total as u64)
            .build()
    }

    pub(crate) fn parent_path(&self) -> &PathBuf {
        &self.parent_path
    }

    pub(crate) fn name(&self) -> Option<&str> {
        self.name.as_deref()
    }
}
