use std::borrow::BorrowMut;
use std::future::Future;
use std::io::{ErrorKind, Result as IoResult};
use std::path::PathBuf;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};

use arrayvec::ArrayString;
use blake3_tree::ProofBuf;
use bytes::{BufMut, BytesMut};
use futures::{ready, FutureExt};
use tokio::io::{AsyncRead, AsyncWrite};

use crate::BLOCKSTORE_PATH;

trait ReadFut: Future<Output = Result<Vec<u8>, std::io::Error>> + Send + Sync {}
impl<T: Future<Output = Result<Vec<u8>, std::io::Error>> + Send + Sync> ReadFut for T {}

/// Leading bit for flagging proofs and chunks.
/// Payloads are either a proof segment (max ~4MiB initial proof),
/// or a chunk (256KiB), so this bit should be safe to use.
const LEADING_BIT: u32 = 1 << 31;

/// Owned blockstore request stream. Responsible for writing a verified
/// stream of blockstore content to `AsyncRead` calls. Drops all writes.
pub struct VerifiedStream {
    current: usize,
    handle: Arc<ContentHandle>,
    read_fut: Option<Pin<Box<dyn ReadFut>>>,
    buffer: BytesMut,
}

impl VerifiedStream {
    pub async fn new(hash: &[u8; 32]) -> Result<Self, std::io::Error> {
        let handle = Arc::new(ContentHandle::load(hash).await?);

        // TODO: Estimate and validate content length limits, based on the
        //       number of chunk hashes in the proof.

        // Create the buffer and write the starting proof to it right away
        let mut buffer = BytesMut::new();
        let proof = ProofBuf::new(handle.tree.as_ref(), 0);
        buffer.put_u32(proof.len() as u32);
        buffer.put_slice(proof.as_slice());

        let current = 0;
        let handle_clone = handle.clone();
        let read_fut = Some(Box::pin(async move { handle_clone.read(current).await }) as _);

        Ok(Self {
            current,
            handle,
            read_fut,
            buffer,
        })
    }
}

impl AsyncRead for VerifiedStream {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context,
        buf: &mut tokio::io::ReadBuf,
    ) -> Poll<IoResult<()>> {
        loop {
            // flush as many bytes as possible
            if !self.buffer.is_empty() {
                let len = buf.remaining().min(self.buffer.len());
                let bytes = self.buffer.split_to(len);
                buf.put_slice(&bytes);

                return Poll::Ready(Ok(()));
            }

            // poll pending read call
            if let Some(fut) = self.read_fut.borrow_mut() {
                match ready!(fut.poll_unpin(cx)) {
                    Ok(block) => {
                        // remove future
                        self.read_fut = None;

                        // write chunk payload (with leading bit set)
                        self.buffer.put_u32(block.len() as u32 | LEADING_BIT);
                        self.buffer.put_slice(&block);

                        // flush read buffer
                        continue;
                    },
                    Err(e) => return Poll::Ready(Err(e)),
                }
            }

            // exit if we finished the last block
            if self.handle.len() == self.current {
                // Since no data was written, this is effectively an EOF signal.
                return Poll::Ready(Ok(()));
            }

            // queue the next block read
            self.current += 1;
            let current = self.current;
            let handle = self.handle.clone();
            let fut = Box::pin(async move { handle.read(current).await });
            self.read_fut = Some(fut);

            // write next proof payload
            let proof = ProofBuf::resume(self.handle.tree.as_ref(), self.current);
            if !proof.is_empty() {
                self.buffer.put_u32(proof.len() as u32);
                self.buffer.put_slice(proof.as_slice());
            }

            continue;
        }
    }
}

// ignore all input
impl AsyncWrite for VerifiedStream {
    fn poll_write(self: Pin<&mut Self>, _cx: &mut Context, buf: &[u8]) -> Poll<IoResult<usize>> {
        Poll::Ready(IoResult::Ok(buf.len()))
    }
    fn poll_flush(self: Pin<&mut Self>, _cx: &mut Context) -> Poll<IoResult<()>> {
        Poll::Ready(IoResult::Ok(()))
    }
    fn poll_shutdown(self: Pin<&mut Self>, _cx: &mut Context) -> Poll<IoResult<()>> {
        Poll::Ready(IoResult::Ok(()))
    }
}

/* Everything after is mostly copied from `lib/fn-sdk/src/blockstore.rs` */

/// Returns the path to a blockstore item with the given hash.
pub fn get_internal_path(hash: &[u8; 32]) -> PathBuf {
    BLOCKSTORE_PATH.join(format!("./internal/{}", to_hex(hash)))
}

/// Returns the path to a blockstore block with the given block counter and hash.
pub fn get_block_path(counter: usize, block_hash: &[u8; 32]) -> PathBuf {
    BLOCKSTORE_PATH.join(format!("./block/{counter}-{}", to_hex(block_hash)))
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

        let vec = HashVec::from_inner(proof);
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

/// Compute the index of a block inside a tree of hashes (`[[u8; 32]]`)
#[inline(always)]
pub fn tree_index(block_counter: usize) -> usize {
    2 * block_counter - block_counter.count_ones() as usize
}

/// A simple static vector of 32-byte hashes, used internally by [`HashTree`].
#[derive(Clone, Hash, PartialEq, Eq, PartialOrd, Ord, Debug)]
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

// TODO(qti3e): Check the safety of this code.
//impl From<Vec<[u8; 32]>> for HashVec {
//    #[inline]
//    fn from(value: Vec<[u8; 32]>) -> Self {
//        let len = value.len() * 32;
//        let ptr = Box::into_raw(value.into_boxed_slice());
//        let buf = unsafe {
//            let slice = std::slice::from_raw_parts_mut(ptr as *mut u8, len);
//            Box::from_raw(slice)
//        };
//        Self { inner: buf }
//    }
//}

impl TryFrom<Vec<u8>> for HashVec {
    type Error = ();

    fn try_from(value: Vec<u8>) -> Result<Self, Self::Error> {
        if value.len() % 32 != 0 {
            return Err(());
        }
        Ok(Self {
            inner: value.into_boxed_slice(),
        })
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
#[derive(Clone, Hash, PartialEq, Eq, PartialOrd, Ord, Debug)]
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
