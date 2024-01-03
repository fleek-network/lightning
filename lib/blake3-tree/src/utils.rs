/// Compute the index of a block inside a tree of hashes (`[[u8; 32]]`)
#[inline(always)]
pub fn tree_index(block_counter: usize) -> usize {
    2 * block_counter - block_counter.count_ones() as usize
}

/// A simple static vector of 32-byte hashes, used internally by [`HashTree`].
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

impl From<&[[u8; 32]]> for HashVec {
    #[inline(always)]
    fn from(value: &[[u8; 32]]) -> Self {
        let len = value.len() * 32;
        let mut buf: Box<[u8]> = vec![0; len].into_boxed_slice();

        // Safety: &[[u8; 32]] has the same layout as &[u8; N * 32], and
        // we allocated a boxed slice exactly that size.
        unsafe {
            std::ptr::copy_nonoverlapping(value.as_ptr() as *const u8, buf.as_mut_ptr(), len);
        }

        Self { inner: buf }
    }
}

impl HashVec {
    /// Create a new HashVec from a slice of 32 byte hashes.
    ///
    /// Safety: This method is unchecked and improper input can result in undefined behavior.
    #[inline(always)]
    pub fn from_inner(inner: Box<[u8]>) -> Self {
        Self { inner }
    }

    /// Get the root hash from the tree
    #[inline(always)]
    pub fn get_root(&self) -> &[u8; 32] {
        // The root hash is always the last 32 bytes in the buffer
        arrayref::array_ref![self.inner, self.inner.len() - 32, 32]
    }

    /// Get the total number of hashes in the tree
    #[inline(always)]
    pub fn len(&self) -> usize {
        self.inner.len() >> 5
    }

    #[inline(always)]
    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }
}

/// A blake3 hash tree of blocks, with a helper index implementation which
/// provides inner block hashes from a block counter.
///
/// Example:
///
/// ```
/// use blake3_tree::utils::HashTree;
/// use fleek_blake3::tree::HashTreeBuilder;
///
/// // Build tree for 1MB of content
/// let mut builder = HashTreeBuilder::new();
/// builder.update(&[0; 1024 * 1024]);
/// let output = builder.finalize();
///
/// // Convert into our util helper
/// let tree: HashTree = output.into();
///
/// // Get some block hashes
/// let block_hash_a = tree[0];
/// let block_hash_b = tree[1];
/// ```
pub struct HashTree {
    inner: HashVec,
}

impl std::ops::Index<usize> for HashTree {
    type Output = [u8; 32];

    /// Returns the hash of `n`-th block.
    #[inline(always)]
    fn index(&self, index: usize) -> &Self::Output {
        &self.inner[tree_index(index)]
    }
}

impl AsRef<HashVec> for HashTree {
    #[inline(always)]
    fn as_ref(&self) -> &HashVec {
        &self.inner
    }
}

impl AsRef<[[u8; 32]]> for HashTree {
    #[inline(always)]
    fn as_ref(&self) -> &[[u8; 32]] {
        self.inner.as_ref()
    }
}

impl From<&[[u8; 32]]> for HashTree {
    #[inline(always)]
    fn from(value: &[[u8; 32]]) -> Self {
        Self {
            inner: value.into(),
        }
    }
}

impl From<fleek_blake3::tree::HashTree> for HashTree {
    #[inline(always)]
    fn from(value: fleek_blake3::tree::HashTree) -> Self {
        value.tree.as_slice().into()
    }
}

impl HashTree {
    /// Create a new HashTree directly from a [`HashVec`]
    /// Safety: This method is unchecked and improper input can result in undefined behavior.
    #[inline(always)]
    pub fn from_inner(inner: HashVec) -> Self {
        Self { inner }
    }

    /// Get the root hash from the tree
    #[inline(always)]
    pub fn get_root(&self) -> &[u8; 32] {
        self.inner.get_root()
    }

    /// Return the number of leaf blocks in this hash tree.
    #[inline(always)]
    pub fn len(&self) -> usize {
        (self.inner.len() + 1) >> 1
    }

    /// Return if the tree is empty
    #[inline(always)]
    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn hashvec_slice_conversions() {
        let test_slice: &[[u8; 32]] = &[[1; 32], [2; 32], [3; 32]];
        let hashvec = super::HashVec::from(test_slice);
        let our_slice = hashvec.as_ref();
        assert_eq!(test_slice, our_slice);
    }
}
