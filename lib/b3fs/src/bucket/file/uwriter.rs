use std::io;

use crate::bucket::{errors, Bucket};

pub struct UntrustedFileWriter {}

impl UntrustedFileWriter {
    pub fn new(bucket: &Bucket, hash: [u8; 32]) -> Self {
        todo!()
    }

    pub async fn feed_proof(&mut self, proof: &[u8]) -> Result<(), errors::FeedProofError> {
        todo!()
    }

    pub async fn write(&mut self, bytes: &[u8]) -> Result<(), errors::WriteError> {
        todo!()
    }

    /// Finalize this write and flush the data to the disk.
    pub async fn commit(self) -> Result<[u8; 32], errors::CommitError> {
        todo!()
    }

    /// Cancel this write and remove anything that this writer wrote to the disk.
    pub async fn rollback(self) -> Result<(), io::Error> {
        todo!()
    }
}
