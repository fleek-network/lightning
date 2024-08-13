use crate::bucket::Bucket;
use crate::directory::{DirectoryHasher, OwnedEntry};

/// A trusted
pub struct DirWriter {
    hasher: DirectoryHasher,
}

impl DirWriter {
    pub fn new(bucket: &Bucket, num_entries: usize) -> Self {
        todo!()
    }

    pub async fn insert(&mut self, entry: OwnedEntry) {}
}
