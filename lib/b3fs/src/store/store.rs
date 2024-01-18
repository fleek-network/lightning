use std::collections::HashMap;
use std::path::{Path, PathBuf};

use bytes::Bytes;
use triomphe::Arc;

use super::directory::Directory;
use super::file::File;
use super::header::Header;
use super::layout::get_header_path;

pub struct Store {
    root_path: PathBuf,
    in_memory_storage: Option<Arc<HashMap<PathBuf, Bytes>>>,
}

impl Store {
    /// Return a new blockstore on the provided root path.
    pub fn new(root_path: PathBuf) -> Self {
        Self {
            root_path,
            in_memory_storage: None,
        }
    }

    /// Create and return an in-memory block store.
    pub fn in_memory() -> Self {
        Self {
            root_path: "/fleek/store".into(),
            in_memory_storage: Some(Arc::new(HashMap::new())),
        }
    }

    /// Returns the root path of the store
    pub fn get_root_path(&self) -> &Path {
        &self.root_path
    }

    /// Read the header of a content with the provided hash. After loading the header you can
    /// see if the given root hash is a file or a directory.
    pub async fn read(&self, hash: [u8; 32]) -> Option<Header> {
        if let Some(store) = &self.in_memory_storage {
            let path = get_header_path(self.root_path.clone(), &hash);
            return store
                .get(&path)
                .map(|data| Header::in_memory(path, data.clone()));
        }
        Header::read(self.root_path.clone(), hash).await.ok()
    }

    /// Read the directory with the given hash from the the blockstore.
    pub async fn read_dir(&self, hash: [u8; 32]) -> Option<Directory> {
        self.read(hash)
            .await
            .and_then(|header| header.as_directory())
    }

    /// Read the file with the given hash from the the blockstore.
    pub async fn read_file(&self, hash: [u8; 32]) -> Option<File> {
        self.read(hash).await.and_then(|header| header.as_file())
    }
}
