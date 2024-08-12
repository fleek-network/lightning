use crate::bucket::Bucket;

pub struct UntrustedFileWriter {}

impl UntrustedFileWriter {
    pub fn new(bucket: &Bucket) -> Self {
        todo!()
    }

    pub async fn feed_proof(&mut self, proof: &[u8]) {}

    pub async fn write(&mut self, bytes: &[u8]) {}
}
