use std::io;

use fleek_blake3::tree::HashTreeBuilder;

use crate::bucket::{errors, Bucket};

pub struct FileWriter {
    hasher: HashTreeBuilder,
}

impl FileWriter {
    pub fn new(bucket: &Bucket) -> Self {
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
