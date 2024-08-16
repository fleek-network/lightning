use crate::bucket::Bucket;
use crate::entry::OwnedEntry;

pub struct UntrustedDirWriter {}

impl UntrustedDirWriter {
    pub fn new(bucket: &Bucket, num_entries: usize) -> Self {
        todo!()
    }

    pub async fn feed_proof(&mut self, proof: &[u8]) {}

    pub async fn insert(&mut self, entry: OwnedEntry) {}
}
