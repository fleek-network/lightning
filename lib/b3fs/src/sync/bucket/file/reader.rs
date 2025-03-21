use std::error;
use std::fs::{self, File};
use std::sync::Arc;

use bytes::BytesMut;

use crate::bucket::{errors, POSITION_START_HASHES};
use crate::collections::tree::AsyncHashTree;
use crate::collections::HashTree;
use crate::hasher::b3::KEY_LEN;
use crate::sync::bucket::errors::ReadError;
use crate::sync::collections::tree::SyncHashTree;

pub struct B3File {
    num_blocks: u32,
    file: fs::File,
}

impl B3File {
    pub(crate) fn new(num_blocks: u32, file: fs::File) -> Self {
        Self { num_blocks, file }
    }

    pub fn num_blocks(&self) -> u32 {
        self.num_blocks
    }

    pub fn hashtree(self) -> Result<SyncHashTree<File>, ReadError> {
        let hash = SyncHashTree::new(self.file, self.num_blocks as usize);
        Ok(hash)
    }
}

#[cfg(test)]
mod tests {
    use std::env::temp_dir;
    use std::fmt::write;
    use std::io::Write;

    use rand::random;
    use triomphe::Arc;

    use super::*;
    use crate::bucket::POSITION_START_HASHES;
    use crate::hasher::b3::CHUNK_START;
    use crate::utils;

    #[test]
    fn test_hashtree() {
        let temp_file_name = random::<[u8; 32]>();
        let temp_dir = temp_dir();
        let file_name = temp_dir.join(format!("test_hashtree_{}", utils::to_hex(&temp_file_name)));
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open(file_name.clone())
            .unwrap();
        let num_blocks = 10;
        let mut data = BytesMut::with_capacity(num_blocks as usize * KEY_LEN);
        data.extend_from_slice(&[0; POSITION_START_HASHES]);
        for _ in 0..(num_blocks * 2) - 1 {
            data.extend_from_slice(&random::<[u8; KEY_LEN]>());
        }
        file.write_all(&data).unwrap();

        let file = fs::File::open(file_name.clone()).unwrap();
        let mut b3file = B3File::new(num_blocks, file);
        let mut hash = b3file.hashtree().unwrap();
        for i in 0..num_blocks {
            let _ = hash.get_hash(i).unwrap();
        }
        let no_more = hash.get_hash(num_blocks + 1).unwrap();
        assert!(no_more.is_none());
        fs::remove_file(file_name).unwrap();
    }
}
