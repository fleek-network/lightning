use std::path::PathBuf;

use blake3_tree::utils::{HashTree, HashVec};
use tokio::{fs, io};

pub struct ContentHandle {
    pub tree: HashTree,
}

impl ContentHandle {
    /// Load the hash tree of a file from the blockstore.
    pub async fn load(hash: &[u8; 32]) -> io::Result<Self> {
        let owned = *hash;
        tokio::task::spawn_blocking(move || Self::load_sync(&owned))
            .await
            .unwrap()
    }

    /// Load the hash tree of a file from the blockstore.
    pub fn load_sync(hash: &[u8; 32]) -> io::Result<Self> {
        let path = get_internal_path(hash);
        let content = std::fs::read(path)?;
        if content.len() & 31 != 0 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Corrupted blockstore",
            ));
        }

        Ok(Self {
            tree: HashTree::from_inner(HashVec::from_inner(content.into_boxed_slice())),
        })
    }

    /// Returns the content for the given block counter.
    ///
    /// # Panics
    ///
    /// If block counter is too large.
    pub async fn get(&self, block_counter: usize) -> io::Result<Vec<u8>> {
        let hash = &self.tree[block_counter];
        let path = get_block_path(block_counter, hash);
        fs::read(path).await
    }

    /// Returns the number of blocks for this content
    #[inline(always)]
    pub fn len(&self) -> usize {
        self.tree.len()
    }

    // TODO: Verify if we actually need this?
    #[inline(always)]
    pub fn is_empty(&self) -> bool {
        self.tree.is_empty()
    }
}

/// Returns the path to a blockstore item with the given hash.
pub fn get_internal_path(hash: &[u8; 32]) -> PathBuf {
    crate::api::blockstore_root().join(format!(
        "./internal/{}",
        fleek_blake3::Hash::from_bytes(*hash).to_hex(),
    ))
}

/// Returns the path to a blockstore block with the given block counter and hash.
pub fn get_block_path(counter: usize, block_hash: &[u8; 32]) -> PathBuf {
    crate::api::blockstore_root().join(format!(
        "./block/{counter}-{}",
        fleek_blake3::Hash::from_bytes(*block_hash).to_hex(),
    ))
}
