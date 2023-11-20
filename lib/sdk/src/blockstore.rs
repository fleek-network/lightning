use std::io::ErrorKind;
use std::path::PathBuf;

use arrayvec::ArrayString;
use blake3_tree::utils::HashTree;

use crate::ipc::BLOCKSTORE;

/// Returns the root blockstore.
///
/// # Panics
///
/// If called from outside of a service execution.
pub fn blockstore_root() -> &'static PathBuf {
    unsafe { BLOCKSTORE.as_ref().expect("setup not completed") }
}

/// Returns the path to a blockstore item with the given hash.
pub fn get_internal_path(hash: &[u8; 32]) -> PathBuf {
    blockstore_root().join(format!("./internal/{}", to_hex(hash)))
}

/// Returns the path to a blockstore block with the given block counter and hash.
pub fn get_block_path(counter: usize, block_hash: &[u8; 32]) -> PathBuf {
    blockstore_root().join(format!("./block/{counter}-{}", to_hex(block_hash)))
}

#[inline]
fn to_hex(slice: &[u8; 32]) -> ArrayString<64> {
    let mut s = ArrayString::new();
    let table = b"0123456789abcdef";
    for &b in slice {
        s.push(table[(b >> 4) as usize] as char);
        s.push(table[(b & 0xf) as usize] as char);
    }
    s
}

/// A handle to some content in the blockstore, providing an easy to use utility for accessing
/// the hash tree and its blocks from the file system.
pub struct ContentHandle {
    pub tree: HashTree,
}

impl ContentHandle {
    /// Load a new content handle, immediately reading the hash tree from the file system.
    pub async fn load(hash: &[u8; 32]) -> std::io::Result<Self> {
        let path = get_internal_path(hash);
        let proof = std::fs::read(path)?.into_boxed_slice();
        if proof.len() & 31 != 0 {
            return Err(ErrorKind::InvalidData.into());
        }

        let vec = blake3_tree::utils::HashVec::from_inner(proof);
        let tree = HashTree::from_inner(vec);

        Ok(Self { tree })
    }

    /// Get the number of blocks for the content.
    #[allow(clippy::len_without_is_empty)]
    pub fn len(&self) -> usize {
        self.tree.len()
    }

    /// Read a block from the file system.
    pub async fn read(&self, block: usize) -> std::io::Result<Vec<u8>> {
        let path = get_block_path(block, &self.tree[block]);
        std::fs::read(path)
    }

    /// Read the entire content from the file system.
    pub async fn read_to_end(&self) -> std::io::Result<Vec<u8>> {
        // Reserve capacity for all but the last block, since we know all blocks but the last one
        // will be 256KiB
        let mut buf = Vec::with_capacity((256 << 10) * (self.len() - 1));
        for i in 0..self.len() {
            buf.append(&mut self.read(i).await?);
        }
        Ok(buf)
    }
}
