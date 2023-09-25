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

pub mod decoder;
pub use decoder::*;

pub mod encoder;
pub use encoder::*;

#[cfg(feature = "tokio")]
pub mod tokio;
#[cfg(feature = "tokio")]
pub use tokio::*;

/// Size of each block in the encoding.
pub const BLOCK_SIZE: usize = 256 * 1024;

/// Maximum bytes a proof can be.
///
/// The maximum theoretical file we support is `2^64` bytes, given we transfer
/// data as blocks of 256KiB (`2^18` bytes) the maximum number of chunks is `2^46`.
/// So the maximum height of the hash tree will be 47. So we will have maximum of
/// 47 hashes (hence `47 * 32`), and one byte per each 8 hash (`ceil(47 / 8) = 6`).
pub const MAX_PROOF_SIZE: usize = 47 * 32 + 6;

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
