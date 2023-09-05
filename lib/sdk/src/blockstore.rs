use std::path::PathBuf;

use tokio::{fs, io};

/// The internal hash tree of a content which
pub struct HashTree {
    inner: HashVec,
}

impl std::ops::Index<usize> for HashTree {
    type Output = [u8; 32];

    /// Returns the hash of `n`-th block.
    #[inline(always)]
    fn index(&self, index: usize) -> &Self::Output {
        let offset = index * 2 - index.count_ones() as usize;
        &self.inner[offset]
    }
}

impl AsRef<HashVec> for HashTree {
    fn as_ref(&self) -> &HashVec {
        &self.inner
    }
}

impl HashTree {
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

        Ok(HashTree {
            inner: HashVec {
                inner: content.into_boxed_slice(),
            },
        })
    }

    /// Returns the content for the given block counter.
    ///
    /// # Panics
    ///
    /// If block counter is too large.
    pub async fn get(&self, block_counter: usize) -> io::Result<Vec<u8>> {
        let hash = &self[block_counter];
        let path = get_block_path(block_counter, hash);
        fs::read(path).await
    }

    /// Return the number of leaf blocks in this hash tree.
    #[inline(always)]
    pub fn len(&self) -> usize {
        (self.inner.len() + 1) >> 1
    }

    #[inline(always)]
    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }
}

/// A simple vector of 32-byte hashes.
pub struct HashVec {
    inner: Box<[u8]>,
}

impl std::ops::Index<usize> for HashVec {
    type Output = [u8; 32];

    #[inline(always)]
    fn index(&self, index: usize) -> &Self::Output {
        arrayref::array_ref![self.inner, index << 5, 32]
    }
}

impl AsRef<[[u8; 32]]> for HashVec {
    #[inline(always)]
    fn as_ref(&self) -> &[[u8; 32]] {
        // Check if number of items divides 32.
        debug_assert_eq!(self.inner.len() & 31, 0);

        // Safety: &[[u8; 32]] has the same layout as &[u8; N * 32], and
        // we know that `inner.len() == 32k`. Also `self.len()` returns
        // the number of bytes divided by 32.
        unsafe { std::slice::from_raw_parts(self.inner.as_ptr() as *const [u8; 32], self.len()) }
    }
}

impl HashVec {
    #[inline(always)]
    pub fn len(&self) -> usize {
        self.inner.len() >> 5
    }

    #[inline(always)]
    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
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
