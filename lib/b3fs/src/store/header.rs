use std::path::PathBuf;

use bytes::Bytes;
use fmmap::tokio::{AsyncMmapFile, AsyncMmapFileExt};

use super::directory::Directory;
use super::file::File;
use super::layout::get_header_path;

pub struct Header {
    pub(crate) content: AsyncMmapFile,
}

impl Header {
    /// Read a directory with the provided hash.
    pub(crate) async fn read(root_path: PathBuf, hash: [u8; 32]) -> fmmap::error::Result<Self> {
        let path = get_header_path(root_path, &hash);
        let content = AsyncMmapFile::open(path).await?;
        Ok(Self { content })
    }

    pub(crate) fn in_memory(path: PathBuf, data: Bytes) -> Self {
        let content = AsyncMmapFile::memory(path, data);
        Self { content }
    }

    #[inline(always)]
    fn tag(&self) -> &[u8] {
        self.content.bytes(0, 4).unwrap_or(&[])
    }

    /// Returns true if this content is a file.
    pub fn is_file(&self) -> bool {
        self.tag() == &[0, 0, 0, 0]
    }

    /// Returns true if this content is a directory.
    pub fn is_directory(&self) -> bool {
        self.tag() == &[1, 0, 0, 0]
    }

    /// If the content is a file converts this header into a [`File`]. Otherwise returns [`None`]
    pub fn as_file(self) -> Option<File> {
        self.is_file().then(|| File::new(self))
    }

    /// If the content is a directory converts this header into a [`Directory`]. Otherwise
    /// returns [`None`].
    pub fn as_directory(self) -> Option<Directory> {
        self.is_directory().then(|| Directory::new(self))
    }
}
