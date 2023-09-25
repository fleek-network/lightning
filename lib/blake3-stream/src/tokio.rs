//! Helpers for interacting with tokio based streams directly.

use std::io::{self, Write};

use arrayref::array_ref;
use blake3_tree::blake3::tree::HashTree;
use blake3_tree::ProofSizeEstimator;
use bytes::{BufMut, BytesMut};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

use super::Encoder;
use crate::{DecoderState, FrameBytes, BLOCK_SIZE, MAX_PROOF_SIZE};

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

pub struct TokioFrameDecoder<R: AsyncRead> {
    reader: R,
    read_buffer: BytesMut,
    block: usize,
    num_blocks: usize,
    length: usize,
    state: DecoderState,
}

impl<R: AsyncRead + Unpin> TokioFrameDecoder<R> {
    /// Create a new stream decoder
    pub fn new(reader: R) -> Self {
        Self {
            reader,
            read_buffer: BytesMut::with_capacity(BLOCK_SIZE),
            block: 0,
            num_blocks: 0,
            length: 0,
            state: DecoderState::WaitingForHeader,
        }
    }
}

impl<R: AsyncRead + Unpin> TokioFrameDecoder<R> {
    pub async fn next_frame(&mut self) -> std::io::Result<Option<FrameBytes>> {
        while let Some(size) = self.state.next_size() {
            if self.read_buffer.len() >= size {
                match self.state {
                    DecoderState::WaitingForHeader => {
                        let bytes = self.read_buffer.split_to(size);
                        self.read_buffer.reserve(size);
                        // read a u64 content length header
                        self.length = u64::from_be_bytes(*array_ref!(bytes, 0, 8)) as usize;
                        self.num_blocks = (self.length + BLOCK_SIZE - 1) / BLOCK_SIZE;
                        self.state = DecoderState::WaitingForProof(
                            ProofSizeEstimator::new(0, self.num_blocks).0,
                        );
                    },
                    DecoderState::WaitingForProof(_) => {
                        let block_len = if self.block < self.num_blocks - 1 {
                            BLOCK_SIZE
                        } else {
                            // final block
                            let mut len = self.length % BLOCK_SIZE;
                            if len == 0 {
                                len = BLOCK_SIZE;
                            }
                            len
                        };
                        self.state = DecoderState::WaitingForBlock(block_len);

                        if size != 0 {
                            let bytes = self.read_buffer.split_to(size);
                            self.read_buffer.reserve(size);
                            return Ok(Some(FrameBytes::Proof(bytes.into())));
                        }
                    },
                    DecoderState::WaitingForBlock(_) => {
                        // we have enough bytes to parse the next item
                        let bytes = self.read_buffer.split_to(size);

                        // setup state for the next block
                        self.block += 1;
                        if self.block < self.num_blocks {
                            self.state = DecoderState::WaitingForProof(
                                ProofSizeEstimator::resume(self.block, self.num_blocks).0,
                            );
                        } else {
                            self.state = DecoderState::Finished;
                        }

                        return Ok(Some(FrameBytes::Chunk(bytes.into())));
                    },
                    DecoderState::Finished => return Ok(None),
                }
            } else {
                // We don't have enough bytes, get some more from the reader
                let mut buf = [0; BLOCK_SIZE];
                match self.reader.read(&mut buf).await? {
                    0 => {
                        return if !self.read_buffer.is_empty() {
                            // If the buffer contains anything, the connection was
                            // interrupted
                            // while transferring data.
                            Err(io::Error::from(io::ErrorKind::ConnectionReset))
                        } else {
                            Ok(None)
                        };
                    },
                    len => {
                        self.read_buffer.extend_from_slice(&buf[0..len]);
                    },
                }
            }
        }

        Ok(None)
    }
}
