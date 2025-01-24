use std::io::ErrorKind;
use std::path::PathBuf;

use b3fs::bucket::Bucket;
use b3fs::collections::tree::AsyncHashTree;
use tokio::fs::File;

use crate::ipc::BLOCKSTORE;

/// Returns the root blockstore.
///
/// # Panics
///
/// If called from outside of a service execution.
pub async fn blockstore_root() -> Bucket {
    let path = unsafe { BLOCKSTORE.as_ref().expect("setup not completed") };
    Bucket::open(path).await.expect("Error opening bucket")
}

pub fn header_file(hash: &[u8; 32]) -> PathBuf {
    let path = unsafe { BLOCKSTORE.as_ref().expect("setup not completed") };
    Bucket::header_path(path, hash).expect("Error opening header path")
}

pub fn block_file(hash: &[u8; 32]) -> PathBuf {
    let path = unsafe { BLOCKSTORE.as_ref().expect("setup not completed") };
    Bucket::block_path(path, hash).expect("Error opening header path")
}

/// A handle to some content in the blockstore, providing an easy to use utility for accessing
/// the hash tree and its blocks from the file system.
pub struct ContentHandle {
    pub bucket: Bucket,
    pub tree: AsyncHashTree<File>,
    pub blocks: u32,
}

fn to_std_io_err<E: ToString>(err: Option<E>, msg: &str) -> std::io::Error {
    let message = if let Some(e) = err {
        format!("{} - Error cause {}", msg, e.to_string())
    } else {
        msg.to_string()
    };
    std::io::Error::new(ErrorKind::Other, message)
}

impl ContentHandle {
    /// Load a new content handle, immediately reading the hash tree from the file system.
    pub async fn load(hash: &[u8; 32]) -> std::io::Result<Self> {
        let bucket = blockstore_root().await;
        let load_content = bucket.get(hash).await?;
        let file_read = load_content.into_file().ok_or(to_std_io_err(
            None as Option<String>,
            "Error converting content to file reader",
        ))?;
        let blocks = file_read.num_blocks();
        let tree = file_read
            .hashtree()
            .await
            .map_err(|e| to_std_io_err(Some(e), "Error getting hashtree from reader"))?;

        Ok(Self {
            bucket,
            tree,
            blocks,
        })
    }

    /// Get the number of blocks for the content.
    #[allow(clippy::len_without_is_empty)]
    pub fn len(&self) -> usize {
        self.blocks as usize
    }

    /// Read a block from the file system.
    pub async fn read(&mut self, block: usize) -> std::io::Result<Vec<u8>> {
        let hash = self
            .tree
            .get_hash(block as u32)
            .await
            .map_err(|e| to_std_io_err(Some(e), "Error getting hash from block"))?
            .ok_or(std::io::ErrorKind::InvalidData)?;
        self.bucket
            .get_block_content(&hash)
            .await
            .map_err(|e| to_std_io_err(Some(e), "Failed to get block content"))?
            .ok_or(to_std_io_err(
                None as Option<String>,
                "Cannot get content from hash",
            ))
    }

    /// Read the entire content from the file system.
    pub async fn read_to_end(&mut self) -> std::io::Result<Vec<u8>> {
        // Reserve capacity for all but the last block, since we know all blocks but the last one
        // will be 256KiB
        let mut buf = Vec::with_capacity((256 << 10) * (self.len() - 1));
        for i in 0..self.len() {
            buf.append(&mut self.read(i).await?);
        }
        Ok(buf)
    }
}
