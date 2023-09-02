//! Verified stream encoding
//!
//! A stream is constructed of a single u64 content length header, blake3 tree segments, and 256 KiB
//! blocks of data.
//!
//! The content length header is used to determine the number of blocks, the length of the last
//! block, and the size of the proof tree. On the client side, a simple state machine is used to
//! determine which type of data is being read.
//!
//! An encoded file will then look like:
//!
//! ```text
//! [ header (u64) . tree bytes . block bytes . block bytes . tree bytes . block bytes ... ]
//! ```

use std::fmt::Debug;
use std::io::{self, Read, Write};

use arrayref::array_ref;
use blake3_tree::blake3::tree::{BlockHasher, HashTree};
use blake3_tree::{IncrementalVerifier, ProofBuf, ProofSizeEstimator};
use bytes::{BufMut, Bytes, BytesMut};

pub const BLOCK_SIZE: usize = 256 * 1024;

pub const PROOF_TAG: u8 = 0x00;
pub const BLOCK_TAG: u8 = 0x01;
pub const SIZED_BLOCK_TAG: u8 = 0x02;

/// Encoder for a blake3 stream of content
pub struct Encoder<W: Write> {
    writer: W,
    buffer: BytesMut,
    tree: HashTree,
    block: usize,
    num_blocks: usize,
    content_len: usize,
}

impl<W: Write> Encoder<W> {
    /// Create a new encoder, immediately writing the u64 length header
    pub fn new(mut writer: W, content_len: usize, tree: HashTree) -> io::Result<Self> {
        writer.write_all(&(content_len as u64).to_be_bytes())?;
        Ok(Self {
            num_blocks: (tree.tree.len() + 1) / 2,
            writer,
            tree,
            content_len,
            buffer: BytesMut::new(),
            block: 0,
        })
    }
}

impl<W: Write> Write for Encoder<W> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.buffer.put(buf);

        let mut proof = if self.block == 0 {
            ProofBuf::new(&self.tree.tree, 0)
        } else {
            ProofBuf::resume(&self.tree.tree, self.block)
        };

        // Write as many blocks as we can
        while !self.buffer.is_empty()
            && (self.buffer.len() >= BLOCK_SIZE
                || ((self.block == self.num_blocks - 1)
                    && self.buffer.len() == self.content_len % BLOCK_SIZE))
        {
            if !proof.is_empty() {
                self.writer.write_all(proof.as_ref())?;
            };

            let bytes = self.buffer.split_to(self.buffer.len().min(BLOCK_SIZE));

            self.writer.write_all(bytes.as_ref())?;

            self.block += 1;
            if self.block < self.num_blocks {
                proof = ProofBuf::resume(&self.tree.tree, self.block)
            }
        }

        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        self.writer.flush()
    }
}

#[derive(Clone, Copy, Debug)]
pub enum DecoderState {
    WaitingForHeader,
    WaitingForProof(usize),
    WaitingForBlock(usize),
    Finished,
}

impl DecoderState {
    pub fn next_size(&self) -> Option<usize> {
        match self {
            DecoderState::WaitingForHeader => Some(8),
            DecoderState::WaitingForProof(proof_len) => Some(*proof_len),
            DecoderState::WaitingForBlock(block_len) => Some(*block_len),
            DecoderState::Finished => None,
        }
    }
}

/// Raw frame decoder that doesn't do any verification, and provides the consumer with proof and
/// content chunk frames to use as is.
pub struct FrameDecoder<R: Read> {
    reader: R,
    read_buffer: BytesMut,
    block: usize,
    num_blocks: usize,
    length: usize,
    state: DecoderState,
}

pub enum FrameBytes {
    Proof(Bytes),
    Chunk(Bytes),
}

impl<R: Read> FrameDecoder<R> {
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

impl<R: Read> FrameDecoder<R> {
    /// Return the next frame in the encoding, reading from the underlying data stream.
    pub fn next_frame(&mut self) -> std::io::Result<Option<FrameBytes>> {
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
                match self.reader.read(&mut buf)? {
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

/// Verified decoder for a blake3 stream of content. Wraps a stream implementing [`std::io::Read`],
/// where each call to [`VerifiedDecoder::read`] writes the verified content to a buffer.
// TODO: return the tree for optional use after
pub struct VerifiedDecoder<R: Read> {
    iv: IncrementalVerifier,
    frame_decoder: FrameDecoder<R>,
    out_buffer: BytesMut,
}

impl<R: Read> VerifiedDecoder<R> {
    /// Create a new stream decoder
    pub fn new(reader: R, root_hash: [u8; 32]) -> Self {
        Self {
            frame_decoder: FrameDecoder::new(reader),
            iv: IncrementalVerifier::new(root_hash, 0),
            out_buffer: BytesMut::new(),
        }
    }
}

impl<R: Read + Debug> Read for VerifiedDecoder<R> {
    /// Read and consumes the underlying stream, writing out only the verified raw content to the
    /// buffer.
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        if self.iv.is_done() {
            return Ok(0);
        }

        if !self.out_buffer.is_empty() {
            let take = self.out_buffer.len().min(buf.len());
            buf[..take].copy_from_slice(&self.out_buffer.split_to(take));
            return Ok(take);
        }

        while let Some(frame) = self.frame_decoder.next_frame()? {
            match frame {
                FrameBytes::Proof(bytes) => {
                    self.iv
                        .feed_proof(&bytes)
                        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
                },
                FrameBytes::Chunk(mut bytes) => {
                    // verify block
                    let mut hasher = BlockHasher::new();
                    hasher.set_block(self.frame_decoder.block - 1);
                    hasher.update(&bytes);
                    self.iv
                        .verify(hasher)
                        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;

                    // write bytes to stream
                    if bytes.len() > buf.len() {
                        // We have to write more bytes than the buffer has available
                        let take = bytes.len() - buf.len();
                        buf[..take].copy_from_slice(&bytes.split_to(take));
                        self.out_buffer.put(bytes);
                        return Ok(take);
                    } else {
                        // or, write the entire content
                        let take = bytes.len().min(buf.len());
                        buf[..take].copy_from_slice(&bytes.split_to(take));
                        return Ok(take);
                    }
                },
            }
        }

        Ok(0)
    }
}

#[cfg(test)]
mod tests {
    use std::io::{Read, Write};

    use blake3_tree::blake3::tree::{HashTree, HashTreeBuilder};
    use bytes::BytesMut;

    use crate::{Encoder, VerifiedDecoder, BLOCK_SIZE};

    pub const TEST_CASES: &[usize] = &[
        BLOCK_SIZE - 1,
        BLOCK_SIZE,
        BLOCK_SIZE + 1,
        2 * BLOCK_SIZE - 1,
        2 * BLOCK_SIZE,
        2 * BLOCK_SIZE + 1,
        3 * BLOCK_SIZE - 1,
        3 * BLOCK_SIZE,
        3 * BLOCK_SIZE + 1,
        4 * BLOCK_SIZE - 1,
        4 * BLOCK_SIZE,
        4 * BLOCK_SIZE + 1,
        8 * BLOCK_SIZE - 1,
        8 * BLOCK_SIZE,
        8 * BLOCK_SIZE + 1,
        16 * BLOCK_SIZE - 1,
        16 * BLOCK_SIZE,
        16 * BLOCK_SIZE + 1,
    ];

    fn get_content_and_tree(len: usize) -> (Vec<u8>, HashTree) {
        let content = vec![0x80; len];
        let mut tree_builder = HashTreeBuilder::new();
        tree_builder.update(&content);

        (content, tree_builder.finalize())
    }

    #[test]
    fn encode_and_decode() -> std::io::Result<()> {
        for &content_len in TEST_CASES {
            let (content, tree) = get_content_and_tree(content_len);

            let mut encoded_buffer = Vec::new();
            let mut encoder = Encoder::new(&mut encoded_buffer, content.len(), tree.clone())?;
            encoder.write_all(&content)?;
            encoder.flush()?;

            let mut decoder = VerifiedDecoder::new(encoded_buffer.as_slice(), tree.hash.into());
            let mut decoded_buffer = Vec::with_capacity(content_len);
            decoder.read_to_end(&mut decoded_buffer)?;

            assert_eq!(content, decoded_buffer);
        }

        Ok(())
    }

    #[test]
    fn encode_incrementally_and_decode() -> std::io::Result<()> {
        for &content_len in TEST_CASES {
            let (content, tree) = get_content_and_tree(content_len);

            let mut encoded_buffer = Vec::new();
            let mut encoder = Encoder::new(&mut encoded_buffer, content.len(), tree.clone())?;

            let mut bytes: BytesMut = content.as_slice().into();
            while !bytes.is_empty() {
                let take = bytes.len().min(BLOCK_SIZE);
                encoder.write_all(&bytes.split_to(take))?;
            }
            encoder.flush()?;

            let mut decoder = VerifiedDecoder::new(encoded_buffer.as_slice(), tree.hash.into());
            let mut decoded_buffer = Vec::with_capacity(content_len);
            decoder.read_to_end(&mut decoded_buffer)?;
            assert_eq!(content, decoded_buffer);
        }

        Ok(())
    }
}
