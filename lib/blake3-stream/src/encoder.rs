use std::io::{self, Write};

use blake3_tree::blake3::tree::HashTree;
use blake3_tree::ProofBuf;
use bytes::{BufMut, BytesMut};

use crate::BLOCK_SIZE;

/// Encoder for a blake3 stream of content
pub struct Encoder<W: Write> {
    pub(crate) writer: W,
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

#[cfg(feature = "tokio")]
pub mod tokio {}
