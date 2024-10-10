use std::array::TryFromSliceError;

use arrayref::array_ref;
use thiserror::Error;
use tokio::io;

use super::b3::KEY_LEN;
use super::byte_hasher::{Blake3Hasher, BlockHasher};
use super::HashTreeCollector;
use crate::bucket::errors;
use crate::utils;

#[derive(Copy, Clone, Debug, Error)]
#[error("Invalid hash length.")]
pub struct InvalidHashSize(#[from] TryFromSliceError);

#[derive(Default, Clone)]
pub struct BufCollector {
    /// The number of items collected so far
    num_collected: usize,
    buffer: Vec<u8>,
}

impl HashTreeCollector for BufCollector {
    #[inline]
    fn push(&mut self, hash: [u8; 32]) {
        self.num_collected += 1;
        self.buffer.extend_from_slice(&hash)
    }

    #[inline]
    fn reserve(&mut self, additional: usize) {
        self.buffer.reserve_exact(additional * KEY_LEN)
    }

    #[inline]
    fn len(&self) -> usize {
        self.num_collected
    }
}

impl BufCollector {
    pub async fn write_hash(
        &mut self,
        mut writer: impl tokio::io::AsyncWriteExt + Unpin,
    ) -> Result<(), io::Error> {
        writer.write_all(&self.buffer).await?;
        writer.flush().await?;
        self.buffer.clear();
        Ok(())
    }

    pub fn get_block_hash(&self, block_counter: usize) -> Option<[u8; 32]> {
        let index = utils::tree_index(block_counter);
        // compute the number of items we have flushed.
        let flushed = self.num_collected - self.buffer.len() / 32;
        if index < flushed {
            return None;
        }
        let offset = index - flushed;
        Some(*array_ref!(&self.buffer, offset * 32, 32))
    }
}
