//! This module provides data structures to represent the UnixFS data model.
use std::path::PathBuf;

use bytes::Bytes;
use ipld_core::cid::Cid;
use ipld_dagpb::PbLink;

use crate::errors::IpldError;

/// A link to another IPLD node.
#[derive(Clone, Debug)]
pub struct Link {
    cid: Cid,
    name: Option<String>,
    size: Option<u64>,
}

impl From<Cid> for Link {
    fn from(cid: Cid) -> Self {
        Self::new(cid, None, None)
    }
}

impl From<&PbLink> for Link {
    fn from(link: &PbLink) -> Self {
        Link::new(link.cid, link.name.clone(), link.size)
    }
}

impl Link {
    pub fn new(cid: Cid, name: Option<String>, size: Option<u64>) -> Self {
        Self { cid, name, size }
    }

    pub fn cid(&self) -> &Cid {
        &self.cid
    }

    pub fn name(&self) -> &Option<String> {
        &self.name
    }

    pub fn size(&self) -> &Option<u64> {
        &self.size
    }

    pub fn get_links(links: &[PbLink]) -> Vec<Link> {
        links.iter().map(Into::into).collect()
    }
}

/// A unique identifier for an IPLD node, which contains a `Link` and a `PathBuf` with the path to
/// this document. If `PathBuf` is empty, then this is the root document.
#[derive(Clone, Debug)]
pub struct DocId {
    cid: Cid,
    path: PathBuf,
}

impl DocId {
    pub fn new(cid: Cid, path: PathBuf) -> Self {
        Self { cid, path }
    }

    pub fn cid(&self) -> &Cid {
        &self.cid
    }

    pub fn path(&self) -> &PathBuf {
        &self.path
    }

    /// Merge the current path with the previous path and the name of the current item.
    ///
    /// This function tries to build the path of the current item by based on the previous item
    pub fn merge(&mut self, previous_item: Option<&PathBuf>, name: Option<&str>) {
        let mut path = previous_item.cloned().unwrap_or_default();
        path.push(self.path());
        if let Some(name) = name {
            path.push(name);
        }
        self.path = path;
    }

    pub fn from_link(link: &Link, current_dir: Option<&DirItem>) -> DocId {
        let mut id = DocId::new(*link.cid(), PathBuf::new());
        id.merge(current_dir.map(|x| x.id().path()), link.name().as_deref());
        id
    }

    pub fn file_name(&self) -> Option<String> {
        self.path
            .file_name()
            .map(|x| x.to_string_lossy().to_string())
    }
}

impl From<Cid> for DocId {
    fn from(cid: Cid) -> Self {
        Self::new(cid, PathBuf::new())
    }
}

impl From<DocId> for Link {
    fn from(id: DocId) -> Self {
        Self::new(id.cid, None, None)
    }
}

/// `DirItem` represents a directory in the IPLD UnixFS data model.
#[derive(Clone)]
pub struct DirItem {
    id: DocId,
    links: Vec<Link>,
}

impl std::fmt::Debug for DirItem {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DirItem")
            .field("id", &self.id)
            .field("links", &self.links.len())
            .finish()
    }
}

impl From<DirItem> for IpldItem {
    fn from(dir: DirItem) -> Self {
        Self::Dir(dir)
    }
}

impl DirItem {
    pub fn id(&self) -> &DocId {
        &self.id
    }

    pub fn links(&self) -> &Vec<Link> {
        &self.links
    }
}

/// `FileItem` represents a file in the IPLD UnixFS data model.
#[derive(Clone)]
pub struct FileItem {
    id: DocId,
    data: Bytes,
}

impl std::fmt::Debug for FileItem {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FileItem")
            .field("id", &self.id)
            .field("data-length", &self.data.len())
            .finish()
    }
}

impl From<FileItem> for IpldItem {
    fn from(file: FileItem) -> Self {
        Self::File(file)
    }
}

impl FileItem {
    pub fn id(&self) -> &DocId {
        &self.id
    }

    pub fn data(&self) -> &Bytes {
        &self.data
    }
}

/// `FileItem` represents a file in the IPLD UnixFS data model.
#[derive(Clone)]
pub struct ChunkItem {
    id: DocId,
    index: usize,
    data: Bytes,
}

impl std::fmt::Debug for ChunkItem {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FileItem")
            .field("id", &self.id)
            .field("index", &self.index)
            .field("data-length", &self.data.len())
            .finish()
    }
}

impl From<ChunkItem> for IpldItem {
    fn from(file: ChunkItem) -> Self {
        Self::Chunk(file)
    }
}

impl ChunkItem {
    pub fn id(&self) -> &DocId {
        &self.id
    }

    pub fn index(&self) -> usize {
        self.index
    }

    pub fn data(&self) -> &Bytes {
        &self.data
    }
}

#[derive(Clone)]
pub struct ChunkFileItem {
    id: DocId,
    chunks: Vec<Link>,
}

impl std::fmt::Debug for ChunkFileItem {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ChunkFileItem")
            .field("id", &self.id)
            .field("chunks", &self.chunks.len())
            .finish()
    }
}

impl From<ChunkFileItem> for IpldItem {
    fn from(chunk: ChunkFileItem) -> Self {
        Self::ChunkedFile(chunk)
    }
}

impl ChunkFileItem {
    pub fn id(&self) -> &DocId {
        &self.id
    }

    pub fn chunks(&self) -> &Vec<Link> {
        &self.chunks
    }
}

#[derive(Clone, Debug)]
pub enum IpldItem {
    Dir(DirItem),
    ChunkedFile(ChunkFileItem),
    File(FileItem),
    Chunk(ChunkItem),
}

impl IpldItem {
    pub fn to_dir(id: DocId, links: Vec<Link>) -> Self {
        let dir = DirItem { id, links };
        Self::Dir(dir)
    }

    pub fn is_dir(&self) -> bool {
        matches!(self, Self::Dir(_))
    }

    pub fn to_chunked_file(id: DocId, chunks: Vec<Link>) -> IpldItem {
        Self::ChunkedFile(ChunkFileItem { id, chunks })
    }

    pub fn to_chunk(id: DocId, index: usize, data: Bytes) -> IpldItem {
        Self::Chunk(ChunkItem { id, index, data })
    }

    pub fn to_file(id: DocId, data: Bytes) -> IpldItem {
        Self::File(FileItem { id, data })
    }

    pub fn links(&self) -> Vec<Link> {
        match self {
            Self::Dir(DirItem { links, .. }) => links.clone(),
            Self::ChunkedFile(ChunkFileItem { chunks, .. }) => chunks.clone(),
            _ => Vec::new(),
        }
    }

    pub fn merge_path(&mut self, path: &PathBuf, name: Option<&str>) {
        match self {
            Self::Dir(DirItem { id, .. }) => id.merge(Some(path), name),
            Self::ChunkedFile(ChunkFileItem { id, .. }) => id.merge(Some(path), name),
            Self::File(FileItem { id, .. }) => id.merge(Some(path), name),
            Self::Chunk(ChunkItem { id, .. }) => id.merge(Some(path), name),
        }
    }

    pub(crate) fn try_into_chunk(&self, i: usize) -> Result<ChunkItem, IpldError> {
        match self {
            Self::File(chunk) => Ok(ChunkItem {
                id: chunk.id.clone(),
                index: i,
                data: chunk.data.clone(),
            }),
            Self::Chunk(chunk) => Ok(chunk.clone()),
            _ => Err(IpldError::InvalidChunk(
                *<Self as Into<DocId>>::into(self.clone()).cid(),
                i,
            )),
        }
    }

    pub(crate) fn try_into_dir(&self) -> Result<DirItem, IpldError> {
        match self {
            Self::Dir(dir) => Ok(dir.clone()),
            _ => Err(IpldError::InvalidDir(
                *<Self as Into<DocId>>::into(self.clone()).cid(),
            )),
        }
    }
}

impl From<IpldItem> for DocId {
    fn from(item: IpldItem) -> Self {
        match item {
            IpldItem::Dir(dir) => dir.id,
            IpldItem::File(file) => file.id,
            IpldItem::ChunkedFile(chunk) => chunk.id,
            IpldItem::Chunk(chunk) => chunk.id,
        }
    }
}

impl From<IpldItem> for Cid {
    fn from(item: IpldItem) -> Self {
        <IpldItem as Into<DocId>>::into(item).cid().into()
    }
}
