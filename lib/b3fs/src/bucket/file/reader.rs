use bytes::BytesMut;
use tokio::fs::{self};
use tokio::io::AsyncReadExt;
use triomphe::Arc;

use crate::bucket::errors;
use crate::collections::tree::AsyncHashTree;
use crate::collections::HashTree;
use crate::hasher::b3::KEY_LEN;

pub struct B3File {
    num_blocks: u32,
    file: Arc<fs::File>,
}

impl B3File {
    pub(crate) fn new(num_blocks: u32, file: Arc<fs::File>) -> Self {
        Self { num_blocks, file }
    }

    pub async fn hashtree(&mut self) -> Result<AsyncHashTree<&mut fs::File>, errors::ReadError> {
        let hash = triomphe::Arc::<tokio::fs::File>::get_mut(&mut self.file)
            .ok_or(errors::ReadError::RefFile)
            .map(|r| AsyncHashTree::new(r, self.num_blocks))?;
        Ok(hash)
    }
}

#[cfg(test)]
mod tests {
    use std::env::temp_dir;
    use std::fmt::write;

    use rand::random;
    use tokio::io::{AsyncRead, AsyncWriteExt};
    use tokio_stream::StreamExt;
    use triomphe::Arc;

    use super::*;
    use crate::utils;

    #[tokio::test]
    async fn test_hashtree() {
        let temp_file_name = random::<[u8; 32]>();
        let temp_dir = temp_dir();
        let file_name = temp_dir.join(format!("test_hashtree_{}", utils::to_hex(&temp_file_name)));
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open(file_name.clone())
            .await
            .unwrap();
        let num_blocks = 10;
        let mut data = BytesMut::with_capacity(num_blocks as usize * KEY_LEN);
        for _ in 0..(num_blocks * 2) - 2 {
            data.extend_from_slice(&random::<[u8; KEY_LEN]>());
        }
        file.write_all(&data).await.unwrap();

        let file = fs::File::open(file_name.clone()).await.unwrap();
        let file_arc = Arc::new(file);
        let mut b3file = B3File::new(num_blocks, file_arc);
        let mut hash = b3file.hashtree().await.unwrap();
        for _ in 0..num_blocks {
            let _ = hash.next().await.unwrap();
        }
        assert!(hash.next().await.is_none());
        fs::remove_file(file_name).await.unwrap();
    }
}
