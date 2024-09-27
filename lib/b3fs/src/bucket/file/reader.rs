use bytes::BytesMut;
use tokio::fs::{self};
use tokio::io::AsyncReadExt;
use triomphe::Arc;

use crate::bucket::errors;
use crate::collections::HashTree;
use crate::hasher::b3::KEY_LEN;

pub struct B3File {
    num_blocks: u32,
    file: Arc<fs::File>,
    content: Vec<[u8; 32]>,
}

impl B3File {
    pub(crate) fn new(num_blocks: u32, file: Arc<fs::File>) -> Self {
        Self {
            num_blocks,
            file,
            content: Vec::with_capacity(num_blocks as usize),
        }
    }

    pub async fn hashtree(&mut self) -> Result<HashTree<'_>, errors::ReadError> {
        let mut read = triomphe::Arc::<tokio::fs::File>::get_mut(&mut self.file)
            .ok_or(errors::ReadError::RefFile)?;
        let mut bytes = BytesMut::with_capacity(KEY_LEN);
        while read.read_exact(&mut bytes).await.is_ok() {
            let mut key = [0u8; KEY_LEN];
            key.copy_from_slice(&bytes);
            self.content.push(key);
            bytes.clear();
        }
        <&Vec<[u8; 32]> as TryInto<HashTree>>::try_into(&self.content)
            .map_err(|_| errors::ReadError::HashTreeConversion)
    }
}
