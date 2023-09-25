//! Helpers for interacting with tokio based streams directly.

use std::io::Write;

use blake3_tree::blake3::tree::HashTree;
use bytes::{BufMut, BytesMut};
use tokio::io::{AsyncWrite, AsyncWriteExt};

use super::Encoder;
use crate::{BLOCK_SIZE, MAX_PROOF_SIZE};

/// A simple wrapper around the [`Encoder`] for directly writing to a tokio stream.
pub struct TokioEncoder<W: AsyncWrite + Unpin> {
    writer: W,
    inner: Encoder<bytes::buf::Writer<BytesMut>>,
}

impl<W: AsyncWrite + Unpin> TokioEncoder<W> {
    pub fn new(writer: W, content_len: usize, tree: HashTree) -> std::io::Result<Self> {
        /// Buffer should be large enough to directly encode an entire block along with a proof, to
        /// avoid additional buffering in the internal encoder.
        const BUFFER_SIZE: usize = BLOCK_SIZE + MAX_PROOF_SIZE;

        let inner = Encoder::new(
            BytesMut::with_capacity(BUFFER_SIZE).writer(),
            content_len,
            tree,
        )?;
        Ok(Self { writer, inner })
    }

    // Encode some raw content, writing complete proofs and chunks to the writer as they come.
    pub async fn write(&mut self, buf: &[u8]) -> std::io::Result<()> {
        // Write all the available bytes to the inner encoder
        self.inner.write_all(buf)?;

        let inner_buf = self.inner.writer.get_mut();
        if !inner_buf.is_empty() {
            // We have stuff to send. Take all available encoded bytes
            let encoded = inner_buf.split();
            // Reserve more for the next write
            inner_buf.reserve(encoded.len());
            // Write them to the stream
            self.writer.write_all(&encoded).await?;
        }

        Ok(())
    }
}
