use fastbloom_rs::{BloomFilter, FilterBuilder};
use phf::calculate_buckets_len;

pub mod iter;
pub mod reader;
mod state;
pub mod uwriter;
pub mod writer;

// Only exposed for benchmarks.
#[doc(hidden)]
pub mod phf;

pub(crate) struct HeaderPositions {
    pub(crate) position_start_hashes: usize,
    pub(crate) length_hashes: usize,
    pub(crate) position_start_bloom_filter: usize,
    pub(crate) bloom_filter_hashes_len: usize,
    pub(crate) bloom_filter_len: usize,
    pub(crate) position_start_phf_table: usize,
    pub(crate) position_start_entries: usize,
    pub(crate) phf_disps_len: usize,
    pub(crate) phf_entries_len: usize,
    pub(crate) phf_key_len: usize,
}

impl HeaderPositions {
    pub(crate) fn new(num_entries: usize) -> Self {
        let position_start_hashes = 4 + 4;
        let length_hashes = num_entries * 32;
        let bloom_filter_hashes_len = 4;
        let bloom_filter_len = FilterBuilder::new(num_entries as u64, 0.01)
            .build_bloom_filter()
            .get_u8_array()
            .len();
        let position_start_bloom_filter = position_start_hashes + length_hashes;
        let position_start_phf_table =
            position_start_bloom_filter + bloom_filter_len + bloom_filter_hashes_len;
        let phf_key_len = 8usize;
        let phf_disps_len = calculate_buckets_len(num_entries as usize) * 4;
        let phf_entries_len = num_entries as usize * 4;
        let length_phf_table = phf_key_len + phf_disps_len + phf_entries_len;
        let position_start_entries = position_start_phf_table + length_phf_table;
        Self {
            position_start_hashes,
            length_hashes,
            position_start_bloom_filter,
            bloom_filter_hashes_len,
            bloom_filter_len,
            position_start_phf_table,
            position_start_entries,
            phf_disps_len,
            phf_entries_len,
            phf_key_len,
        }
    }

    pub(crate) fn bloom_filter_start_entries(&self) -> usize {
        self.position_start_bloom_filter + self.bloom_filter_hashes_len
    }

    pub(crate) fn phf_table_start_disps(&self) -> usize {
        self.position_start_phf_table + self.phf_key_len
    }

    pub(crate) fn phf_table_start_entries(&self) -> usize {
        self.position_start_phf_table + self.phf_key_len + self.phf_disps_len
    }
}

#[cfg(test)]
mod tests {
    use std::env::temp_dir;

    use super::*;
    use crate::bucket::Bucket;
    use crate::hasher::dir_hasher::DirectoryHasher;
    use crate::test_utils::*;

    pub(super) async fn setup_bucket() -> Result<Bucket, Box<dyn std::error::Error>> {
        let temp_dir = temp_dir().join("b3fs_tests");
        tokio::fs::create_dir_all(&temp_dir).await?;
        Ok(Bucket::open(&temp_dir).await?)
    }
}
