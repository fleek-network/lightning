use crate::bucket::Bucket;
use crate::entry::OwnedEntry;

/// A trusted directory writer.
pub struct DirWriter {}

impl DirWriter {
    pub fn new(bucket: &Bucket, num_entries: usize) -> Self {
        todo!()
    }

    pub async fn insert(&mut self, entry: OwnedEntry) {}
}
