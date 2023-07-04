//! Verified stream encoding
//!
//! A stream is constructed of blake3 tree segments and blocks of 256 KiB.
//!
//! The encoding is divided up into a few frames:
//!
//! - Tree Segment:
//!
//!   ```text
//!   [ 0x00 . len (u16) . proof bytes ]
//!   ```
//!
//! - Block:
//!
//!   ```text
//!   [ 0x01 . block bytes (256KiB) ]
//!   ```
//!
//! - Sized Block:
//!
//!   ```text
//!   [ 0x02 . byte len (u32) . block bytes ]
//!   ```
//!
//! An encoded file will then look like:
//!
//! ```text
//! [ tree segment ] [ block ] [ block ] ... [ sized block ]
//! ```
//!
//! # TODO
//!
//! Frame padding (tag, lengths) can be eliminated by prefixing the stream with a fixed size content
//! length header. From this length, we should be able to calculate the number of blocks, and the
//! final blocks length. Additionally, from the current block it should be possible to calculate the
//! tree segment size. We can also keep a state enum, which would dictate if a header, tree segment,
//! or block should be read next.
//!
//! An encoded file would then look like:
//!
//! ```text
//! [ header (u64) . tree segment bytes . block bytes . block bytes ... ]
//! ```

use std::{
    fmt::Debug,
    io::{self, Read, Write},
};

use arrayref::array_ref;
use blake3_tree::{
    blake3::tree::{BlockHasher, HashTree},
    IncrementalVerifier, ProofBuf,
};
use bytes::{Buf, BufMut, BytesMut};

pub const BLOCK_SIZE: usize = 256 * 1024;

pub const PROOF_TAG: u8 = 0x00;
pub const BLOCK_TAG: u8 = 0x01;
pub const SIZED_BLOCK_TAG: u8 = 0x02;

/// Frames used in the encoding
pub enum Frame {
    TreeSegment(BytesMut),
    Block(BytesMut),
}

impl Frame {
    /// Return the calculated size of the encoded frame
    pub fn size(&self) -> usize {
        match self {
            Self::TreeSegment(bytes) => 3 + bytes.len(),
            Self::Block(bytes) => {
                if bytes.len() == BLOCK_SIZE {
                    1 + BLOCK_SIZE
                } else {
                    5 + bytes.len()
                }
            },
        }
    }

    /// Encode the frame
    pub fn encode(self) -> BytesMut {
        match self {
            Frame::TreeSegment(bytes) => {
                let mut buf = BytesMut::with_capacity(3 + bytes.len());
                buf.put_u8(PROOF_TAG);
                buf.put_u16(bytes.len() as u16);
                buf.put(bytes);
                buf
            },
            Frame::Block(bytes) => {
                if bytes.len() == BLOCK_SIZE {
                    let mut buf = BytesMut::with_capacity(1 + BLOCK_SIZE);
                    buf.put_u8(BLOCK_TAG);
                    buf.put(bytes.as_ref());
                    buf
                } else {
                    let mut buf = BytesMut::with_capacity(5 + bytes.len());
                    buf.put_u8(SIZED_BLOCK_TAG);
                    buf.put_u32(bytes.len() as u32);
                    buf.put(bytes.as_ref());
                    buf
                }
            },
        }
    }

    /// Decode a frame
    pub fn decode(buf: &mut BytesMut) -> io::Result<Option<Self>> {
        if buf.is_empty() {
            return Ok(None);
        }

        match buf[0] {
            PROOF_TAG => {
                if buf.len() >= 3 {
                    let len = u16::from_be_bytes(*array_ref!(buf, 1, 2)) as usize;
                    if buf.len() >= len + 3 {
                        buf.advance(3);
                        let bytes = buf.split_to(len);
                        buf.reserve(len + 3);
                        return Ok(Some(Self::TreeSegment(bytes)));
                    }
                }
                Ok(None)
            },
            BLOCK_TAG => {
                if buf.len() > BLOCK_SIZE {
                    buf.advance(1);
                    let bytes = buf.split_to(BLOCK_SIZE);
                    buf.reserve(BLOCK_SIZE + 1);
                    Ok(Some(Self::Block(bytes)))
                } else {
                    Ok(None)
                }
            },
            SIZED_BLOCK_TAG => {
                if buf.len() >= 5 {
                    let len = u32::from_be_bytes(*array_ref!(buf, 1, 4)) as usize;
                    if buf.len() >= len + 5 {
                        buf.advance(5);
                        let bytes = buf.split_to(len);
                        buf.reserve(5 + len);
                        return Ok(Some(Self::Block(bytes)));
                    }
                }
                Ok(None)
            },
            _ => Err(io::Error::from(io::ErrorKind::InvalidData)),
        }
    }
}

/// Encoder for a blake3 stream of content
pub struct Encoder<W: Write> {
    writer: W,
    tree: HashTree,
    buffer: BytesMut,
    block: usize,
    num_blocks: usize,
}

impl<W: Write> Encoder<W> {
    pub fn new(writer: W, tree: HashTree) -> Self {
        let max_block = (tree.tree.len() + 1) / 2;
        Self {
            writer,
            tree,
            buffer: BytesMut::new(),
            block: 0,
            num_blocks: max_block,
        }
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
            && (self.buffer.len() >= BLOCK_SIZE || self.block == self.num_blocks - 1)
        {
            if !proof.is_empty() {
                let bytes = Frame::TreeSegment(proof.as_slice().into()).encode();
                self.writer.write_all(&bytes)?;
            };

            let bytes = self.buffer.split_to(self.buffer.len().min(BLOCK_SIZE));
            let encoded = Frame::Block(bytes).encode();

            self.writer.write_all(&encoded)?;

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

/// Decoder for a blake3 stream of content
/// TODO:
///   - make verification optional
///   - return the tree for optional use after
pub struct VerifiedDecoder<R: Read> {
    reader: R,
    iv: IncrementalVerifier,
    read_buffer: BytesMut,
    out_buffer: BytesMut,
    block: usize,
}

impl<R: Read> VerifiedDecoder<R> {
    /// Create a new stream decoder
    pub fn new(reader: R, root_hash: [u8; 32]) -> Self {
        Self {
            reader,
            iv: IncrementalVerifier::new(root_hash, 0),
            read_buffer: BytesMut::with_capacity(BLOCK_SIZE + 4),
            out_buffer: BytesMut::new(),
            block: 0,
        }
    }
}

impl<R: Read + Debug> Read for VerifiedDecoder<R> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        if self.iv.is_done() {
            return Ok(0);
        }

        if !self.out_buffer.is_empty() {
            let take = self.out_buffer.len().min(buf.len());
            buf[..take].copy_from_slice(&self.out_buffer.split_to(take));
            return Ok(take);
        }

        loop {
            if let Some(frame) = Frame::decode(&mut self.read_buffer)? {
                match frame {
                    Frame::TreeSegment(bytes) => {
                        self.iv
                            .feed_proof(&bytes)
                            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
                    },
                    Frame::Block(mut bytes) => {
                        // verify block
                        let mut hasher = BlockHasher::new();
                        hasher.set_block(self.block);
                        hasher.update(&bytes);

                        self.iv
                            .verify(hasher)
                            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;

                        self.block += 1;

                        if bytes.len() > buf.len() {
                            // We have to write more bytes than the buffer has available
                            let take = bytes.len() - buf.len();
                            buf[..take].copy_from_slice(&bytes.split_to(take));
                            self.out_buffer.put(bytes);
                            break Ok(take);
                        } else {
                            // or, write the entire content
                            let take = bytes.len().min(buf.len());
                            buf[..take].copy_from_slice(&bytes.split_to(take));
                            break Ok(take);
                        }
                    },
                }
            } else {
                // Couldn't decode a frame, get some more bytes from the reader
                let mut buf = [0; BLOCK_SIZE];
                match self.reader.read(&mut buf)? {
                    0 => {
                        break if !self.read_buffer.is_empty() {
                            // If the buffer contains anything, the connection was
                            // interrupted
                            // while transferring data.
                            Err(io::Error::from(io::ErrorKind::ConnectionReset))
                        } else {
                            Ok(0)
                        };
                    },
                    len => {
                        self.read_buffer.extend_from_slice(&buf[0..len]);
                    },
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::io::{Read, Write};

    use blake3_tree::blake3::tree::{HashTree, HashTreeBuilder};
    use bytes::BytesMut;

    use crate::{Encoder, VerifiedDecoder, BLOCK_SIZE};

    pub const TEST_CASES: &[usize] = &[
        1,
        1024,
        16 * 1024,
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
    fn encode_decode() -> std::io::Result<()> {
        for &content_len in TEST_CASES {
            let (content, tree) = get_content_and_tree(content_len);

            let mut encoded_buffer = Vec::new();
            let mut encoder = Encoder::new(&mut encoded_buffer, tree.clone());
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
    fn encode_content_incrementally() -> std::io::Result<()> {
        for &content_len in TEST_CASES {
            let (content, tree) = get_content_and_tree(content_len);

            let mut encoded_buffer = Vec::new();
            let mut encoder = Encoder::new(&mut encoded_buffer, tree.clone());

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
