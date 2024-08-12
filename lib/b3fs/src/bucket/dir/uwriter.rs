use crate::bucket::Bucket;
use crate::directory::OwnedEntry;

pub struct UntrustedDirWriter {}

impl UntrustedDirWriter {
    pub fn new(bucket: &Bucket) -> Self {
        todo!()
    }

    pub async fn feed_proof(&mut self, proof: &[u8]) {}

    pub async fn insert(&mut self, entry: OwnedEntry) {}
}
