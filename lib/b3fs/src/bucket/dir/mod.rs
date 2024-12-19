use fastbloom_rs::{BloomFilter, FilterBuilder};
use phf::{calculate_buckets_len, PHF_TABLE_RANDOMIZED_KEY_SIZE};

use super::POSITION_START_HASHES;

pub mod iter;
pub mod reader;
mod state;
pub mod uwriter;
pub mod writer;

// Only exposed for benchmarks.
#[doc(hidden)]
pub mod phf;

/// Positions of different sections in the directory file header
#[derive(Debug)]
pub(crate) struct HeaderPositions {
    /// Length of the hash entries section in bytes
    pub(crate) length_hashes: usize,
    /// Starting position of the Bloom filter section
    pub(crate) position_start_bloom_filter: usize,
    /// Number of hash functions used by the Bloom filter
    pub(crate) bloom_filter_hashes_len: usize,
    /// Size of the Bloom filter in bytes
    pub(crate) bloom_filter_len: usize,
    /// Starting position of the PHF (Perfect Hash Function) table
    pub(crate) position_start_phf_table: usize,
    /// Starting position of the directory entries
    pub(crate) position_start_entries: usize,
    /// Length of the PHF displacement array in bytes
    pub(crate) phf_disps_len: usize,
    /// Length of the PHF entries array in bytes
    pub(crate) phf_entries_len: usize,
}

impl HeaderPositions {
    /// Creates a new HeaderPositions struct for a directory with the given number of entries
    ///
    /// # Arguments
    /// * `num_entries` - Number of entries in the directory
    ///
    /// # Returns
    /// A new HeaderPositions struct with calculated positions for all sections
    pub(crate) fn new(num_entries: usize) -> Self {
        let length_hashes = num_entries * 64 - 32;
        let bloom_filter_hashes_len = 4;
        let bloom_filter_len = FilterBuilder::new(num_entries as u64, 0.01)
            .build_bloom_filter()
            .get_u8_array()
            .len();
        let position_start_bloom_filter = POSITION_START_HASHES + length_hashes;
        let position_start_phf_table =
            position_start_bloom_filter + bloom_filter_len + bloom_filter_hashes_len;
        let phf_disps_len = calculate_buckets_len(num_entries) * 4;
        let phf_entries_len = num_entries * 4;
        let length_phf_table = PHF_TABLE_RANDOMIZED_KEY_SIZE + phf_disps_len + phf_entries_len;
        let position_start_entries = position_start_phf_table + length_phf_table;
        Self {
            length_hashes,
            position_start_bloom_filter,
            bloom_filter_hashes_len,
            bloom_filter_len,
            position_start_phf_table,
            position_start_entries,
            phf_disps_len,
            phf_entries_len,
        }
    }

    /// Returns the starting position of the Bloom filter entries
    pub(crate) fn bloom_filter_start_entries(&self) -> usize {
        self.position_start_bloom_filter + self.bloom_filter_hashes_len
    }

    /// Returns the starting position of the PHF displacement array
    pub(crate) fn phf_table_start_disps(&self) -> usize {
        self.position_start_phf_table + PHF_TABLE_RANDOMIZED_KEY_SIZE
    }

    /// Returns the starting position of the PHF entries array
    pub(crate) fn phf_table_start_entries(&self) -> usize {
        self.position_start_phf_table + PHF_TABLE_RANDOMIZED_KEY_SIZE + self.phf_disps_len
    }
}

#[cfg(test)]
mod tests {
    use std::env::temp_dir;

    use super::*;
    use crate::bucket::Bucket;
    use crate::hasher::dir_hasher::DirectoryHasher;
    use crate::test_utils::*;

    /// Sets up a test bucket in a temporary directory
    pub(super) async fn setup_bucket() -> Result<Bucket, Box<dyn std::error::Error>> {
        let temp_dir = temp_dir().join("b3fs_tests");
        tokio::fs::create_dir_all(&temp_dir).await?;
        Ok(Bucket::open(&temp_dir).await?)
    }
}
