//! Forms a post-order binary tree over a flat hash slice.

use std::fmt::Debug;
use std::future::Future;
use std::ops::Index;
use std::pin::Pin;
use std::task::{Context, Poll};

use tokio::io::{AsyncReadExt, AsyncSeekExt};
use tokio_stream::Stream;

use super::error::CollectionTryFromError;
use super::flat::FlatHashSlice;
use crate::bucket::POSITION_START_HASHES;
use crate::stream::buffer::ProofBuf;
use crate::stream::walker::Mode;
use crate::utils::{is_valid_tree_len, tree_index};

/// A wrapper around a list of hashes that provides access only to the leaf nodes in the tree.
#[derive(Clone, Copy)]
pub struct HashTree<'s> {
    inner: FlatHashSlice<'s>,
}

/// An iterator over a [`HashTree`] which iterates over the leaf nodes of a tree.
pub struct HashTreeIter<'t> {
    forward: usize,
    backward: usize,
    tree: HashTree<'t>,
}

impl<'s> TryFrom<FlatHashSlice<'s>> for HashTree<'s> {
    type Error = CollectionTryFromError;

    #[inline]
    fn try_from(value: FlatHashSlice<'s>) -> Result<Self, Self::Error> {
        if !is_valid_tree_len(value.len()) {
            Err(CollectionTryFromError::InvalidHashCount)
        } else {
            Ok(Self { inner: value })
        }
    }
}

impl<'s> TryFrom<&'s [u8]> for HashTree<'s> {
    type Error = CollectionTryFromError;

    #[inline]
    fn try_from(value: &'s [u8]) -> Result<Self, Self::Error> {
        Self::try_from(FlatHashSlice::try_from(value)?)
    }
}

impl<'s> TryFrom<&'s [[u8; 32]]> for HashTree<'s> {
    type Error = CollectionTryFromError;

    #[inline]
    fn try_from(value: &'s [[u8; 32]]) -> Result<Self, Self::Error> {
        Self::try_from(FlatHashSlice::from(value))
    }
}

impl<'s> TryFrom<&'s Vec<[u8; 32]>> for HashTree<'s> {
    type Error = CollectionTryFromError;

    #[inline]
    fn try_from(value: &'s Vec<[u8; 32]>) -> Result<Self, Self::Error> {
        Self::try_from(FlatHashSlice::from(value))
    }
}

impl<'s> Index<usize> for HashTree<'s> {
    type Output = [u8; 32];

    #[inline]
    fn index(&self, index: usize) -> &Self::Output {
        if index >= self.len() {
            // TODO(qti3e): is this check necessary?
            panic!("Out of bound.");
        }

        &self.inner[tree_index(index)]
    }
}

impl<'s> IntoIterator for HashTree<'s> {
    type Item = &'s [u8; 32];
    type IntoIter = HashTreeIter<'s>;
    fn into_iter(self) -> Self::IntoIter {
        HashTreeIter::new(self)
    }
}

impl<'s> HashTree<'s> {
    #[inline]
    pub fn root(&self) -> &'s [u8; 32] {
        self.inner.get(self.inner.len() - 1)
    }

    /// Returns the number of items in this hash tree.
    #[inline]
    pub fn len(&self) -> usize {
        (self.inner.len() + 1) >> 1
    }

    /// Returns the total number of hashes making up this tree.
    #[inline]
    pub fn inner_len(&self) -> usize {
        self.inner.len()
    }

    /// A hash tree is never empty.
    #[inline(always)]
    pub fn is_empty(&self) -> bool {
        false
    }

    /// Returns the internal representation of the hash tree which is a flat hash slice.
    #[inline(always)]
    pub fn as_inner(&self) -> &FlatHashSlice {
        &self.inner
    }

    /// Shorthand for [`ProofBuf::new`].
    #[inline]
    pub fn generate_proof(&self, mode: Mode, index: usize) -> ProofBuf {
        ProofBuf::new(mode, *self, index)
    }
}

impl<'s> HashTreeIter<'s> {
    fn new(tree: HashTree<'s>) -> Self {
        Self {
            forward: 0,
            backward: tree.len(),
            tree,
        }
    }

    #[inline(always)]
    fn is_done(&self) -> bool {
        self.forward >= self.backward
    }
}

impl<'s> Iterator for HashTreeIter<'s> {
    type Item = &'s [u8; 32];

    fn next(&mut self) -> Option<Self::Item> {
        if self.is_done() {
            return None;
        }
        let idx = tree_index(self.forward);
        self.forward += 1;
        Some(self.tree.inner.get(idx))
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let r = self.backward.saturating_sub(self.forward);
        (r, Some(r))
    }

    fn nth(&mut self, n: usize) -> Option<Self::Item> {
        self.forward += n;
        self.next()
    }
}

impl<'s> DoubleEndedIterator for HashTreeIter<'s> {
    fn next_back(&mut self) -> Option<Self::Item> {
        if self.is_done() {
            return None;
        }
        self.backward -= 1;
        let idx = tree_index(self.backward);
        Some(self.tree.inner.get(idx))
    }

    fn nth_back(&mut self, n: usize) -> Option<Self::Item> {
        self.backward = self.backward.saturating_sub(n);
        self.next()
    }
}

impl<'s> Debug for HashTree<'s> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        super::printer::print(self, f)
    }
}

enum State {
    SeekPosition,
    Reading,
    ValueReady,
}

pub struct AsyncHashTree<T: AsyncReadExt + AsyncSeekExt + Unpin> {
    file_reader: T,
    number_of_hashes: u32,
    current_block: u32,
    current_hash: [u8; 32],
    state: State,
}

/// An asynchronous stream that reads hashes from a file.
impl<T> AsyncHashTree<T>
where
    T: AsyncReadExt + AsyncSeekExt + Unpin,
{
    pub fn new(file_reader: T, number_of_hashes: u32) -> Self {
        Self {
            file_reader,
            number_of_hashes,
            current_block: 0,
            current_hash: [0; 32],
            state: State::SeekPosition,
        }
    }
}

impl<T> Stream for AsyncHashTree<T>
where
    T: AsyncReadExt + AsyncSeekExt + Unpin,
{
    type Item = Result<[u8; 32], std::io::Error>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let this = self.as_mut().get_mut();

        if this.current_block >= this.number_of_hashes {
            return Poll::Ready(None);
        }

        loop {
            match this.state {
                State::SeekPosition => {
                    let index = tree_index(this.current_block as usize) as u64 * 32 + POSITION_START_HASHES as u64;
                    let seek_future = this.file_reader.seek(tokio::io::SeekFrom::Start(index));
                    let mut seek_future = std::pin::pin!(seek_future);

                    match seek_future.as_mut().poll(cx) {
                        Poll::Ready(Ok(_)) => {
                            this.state = State::Reading;
                        },
                        Poll::Ready(Err(e)) => return Poll::Ready(Some(Err(e))),
                        Poll::Pending => return Poll::Pending,
                    }
                },
                State::Reading => {
                    let read_future = this.file_reader.read_exact(&mut this.current_hash);
                    let mut read_future = std::pin::pin!(read_future);
                    match read_future.as_mut().poll(cx) {
                        Poll::Ready(Ok(_)) => {
                            this.state = State::ValueReady;
                        },
                        Poll::Ready(Err(e)) => return Poll::Ready(Some(Err(e))),
                        Poll::Pending => return Poll::Pending,
                    }
                },
                State::ValueReady => {
                    this.current_block += 1;
                    this.state = State::SeekPosition;
                    return Poll::Ready(Some(Ok(this.current_hash)));
                },
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::fmt::write;
    use std::io::Seek;

    use rand::random;
    use tokio::io::{AsyncRead, AsyncSeek};
    use tokio_stream::StreamExt;

    use super::*;

    struct MockReader {
        data: Vec<u8>,
        position: usize,
    }

    impl MockReader {
        fn new(data: Vec<u8>) -> Self {
            Self { data, position: 0 }
        }
    }

    impl AsyncRead for MockReader {
        fn poll_read(
            mut self: Pin<&mut Self>,
            _: &mut Context<'_>,
            buf: &mut tokio::io::ReadBuf<'_>,
        ) -> Poll<std::io::Result<()>> {
            let remaining = self.data.len() - self.position;
            let to_read = std::cmp::min(remaining, buf.remaining());
            buf.put_slice(&self.data[self.position..self.position + to_read]);
            self.position += to_read;
            Poll::Ready(Ok(()))
        }
    }

    impl AsyncSeek for MockReader {
        fn start_seek(self: Pin<&mut Self>, position: std::io::SeekFrom) -> std::io::Result<()> {
            Ok(())
        }

        fn poll_complete(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<std::io::Result<u64>> {
            Poll::Ready(Ok(self.position as u64))
        }
    }

    #[tokio::test]
    async fn test_single_hash() {
        let data = random::<[u8; 32]>();
        let reader = MockReader::new(data.clone().to_vec());
        let mut tree = AsyncHashTree::new(reader, 1);

        let r = tree.next().await.unwrap();
        assert!(r.is_ok());
        assert_eq!(r.unwrap(), data);
        let r = tree.next().await;
        assert!(r.is_none());
    }

    #[tokio::test]
    async fn test_multiple_hashes() {
        let data1 = random::<[u8; 32]>();
        let data2 = random::<[u8; 32]>();
        let data = data1
            .iter()
            .chain(data1.iter())
            .chain(data2.iter())
            .cloned()
            .collect::<Vec<_>>();
        let reader = MockReader::new(data);
        let mut tree = AsyncHashTree::new(reader, 3);

        let r = tree.next().await.unwrap();
        assert!(r.is_ok());
        assert_eq!(r.unwrap(), data1);
        let r = tree.next().await.unwrap();
        assert!(r.is_ok());
        assert_eq!(r.unwrap(), data1);
        let r = tree.next().await.unwrap();
        assert!(r.is_ok());
        assert_eq!(r.unwrap(), data2);
        let r = tree.next().await;
        assert!(r.is_none());
    }

    #[tokio::test]
    async fn test_incomplete_hash() {
        let data = random::<[u8; 31]>(); // Not enough data for a full hash
        let reader = MockReader::new(data.to_vec());
        let mut tree = AsyncHashTree::new(reader, 1);

        let r = tree.next().await.unwrap();
        assert!(r.is_err()); // Should return an error
    }

    #[tokio::test]
    async fn test_more_hashes_than_available() {
        let data1 = random::<[u8; 32]>();
        let data2 = random::<[u8; 32]>();
        let binding = [data1, data2];
        let data = binding.iter().flatten(); // Two complete hashes
        let reader = MockReader::new(data.cloned().collect());
        let mut tree = AsyncHashTree::new(reader, 3); // Ask for 3 hashes

        let r = tree.next().await.unwrap();
        assert!(r.is_ok());
        assert_eq!(r.unwrap(), data1);
        let r = tree.next().await.unwrap();
        assert!(r.is_ok());
        assert_eq!(r.unwrap(), data2);
        let r = tree.next().await.unwrap();
        assert!(r.is_err()); // Should return None after reading available hashes
    }
}
