//! B3 directory reader implementation.
//!
//! This module provides functionality for reading B3 directory files, including:
//! - Reading directory entries by name using a perfect hash function (PHF) and bloom filter
//! - Iterating through all directory entries
//! - Accessing the directory's hash tree
//! - Reading symlinks and regular file entries

use std::borrow::Borrow;
use std::cell::Cell;
use std::io::SeekFrom;
use std::mem;
use std::ops::Deref;

use arrayref::array_ref;
use fastbloom_rs::{BloomFilter, Membership};
use tokio::fs::{self};
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncSeekExt, BufReader};
use tokio::sync::OnceCell;
use triomphe::Arc;

use super::iter::DirEntriesIter;
use super::phf::PHF_TABLE_RANDOMIZED_KEY_SIZE;
use crate::bucket::dir::phf::{calculate_buckets_len, displace, hash, HasherState};
use crate::bucket::dir::HeaderPositions;
use crate::bucket::{errors, POSITION_START_HASHES};
use crate::collections::tree::AsyncHashTree;
use crate::collections::HashTree;
use crate::entry::{BorrowedEntry, BorrowedLink, OwnedLink};
use crate::hasher::dir_hasher::B3_DIR_IS_SYM_LINK;

/// A B3 directory reader.
///
/// Provides methods to read entries from a B3 directory file, including lookup by name
/// and iteration over all entries.
pub struct B3Dir {
    /// The number of entries in this directory.
    num_entries: u16,
    /// The actual header file.
    file: fs::File,
    /// Bloom filter for fast negative lookups
    bloom_filter: OnceCell<BloomFilter>,
    /// Directory header positions
    positions: HeaderPositions,
    /// Perfect hash function table
    phf_table: OnceCell<HasherState>,
}

/// Macro to handle EOF conditions when reading entries
#[macro_use]
macro_rules! check_eof {
    ($flag:expr) => {
        if let Err(e) = $flag {
            if e.kind() == std::io::ErrorKind::UnexpectedEof {
                return Ok(None);
            } else {
                return Err(errors::ReadError::from(e));
            }
        }
    };
}

impl B3Dir {
    /// Creates a new B3Dir instance.
    ///
    /// # Arguments
    /// * `num_entries` - Number of entries in the directory
    /// * `file` - The directory header file
    pub(crate) fn new(num_entries: u32, file: fs::File) -> Self {
        debug_assert!(num_entries <= u16::MAX as u32);
        let positions = HeaderPositions::new(num_entries as usize);
        Self {
            num_entries: num_entries as u16,
            file,
            bloom_filter: OnceCell::new(),
            phf_table: OnceCell::new(),
            positions,
        }
    }

    /// Gets the directory's hash tree.
    ///
    /// # Returns
    /// The hash tree containing entry hashes, or an error if reading fails.
    pub async fn hashtree(&mut self) -> Result<AsyncHashTree<fs::File>, errors::ReadError> {
        let file = self.file.try_clone().await?;
        let hash = AsyncHashTree::new(file, self.num_entries as usize);
        Ok(hash)
    }

    /// Looks up a directory entry by name.
    ///
    /// Uses bloom filter for fast negative lookups and PHF for exact matching.
    ///
    /// # Arguments
    /// * `name` - Name of the entry to look up
    ///
    /// # Returns
    /// The entry if found, None if not found, or an error if reading fails.
    pub async fn get_entry<'a>(
        &'a mut self,
        name: &'a [u8],
    ) -> Result<Option<BorrowedEntry<'a>>, errors::ReadError> {
        let bloom_filter = self.get_bloom_filter().await?;
        if bloom_filter.contains(name) {
            let phf_table = self.get_phf_table().await?;
            let entry_offset = self.get_entry_offset(name, phf_table).await?;
            return self.read_entry_at_offset(name, entry_offset).await;
        }
        Ok(None)
    }

    /// Creates an iterator over all directory entries.
    ///
    /// # Returns
    /// An iterator over all entries, or an error if creating the iterator fails.
    pub async fn entries(&self) -> Result<DirEntriesIter, errors::ReadError> {
        let result = DirEntriesIter::new(
            self.file.try_clone().await?,
            self.positions.position_start_entries as u64,
        )
        .await?;
        Ok(result)
    }

    /// Creates a new file handle positioned at the given offset.
    async fn position_file(&self, position: u64) -> Result<fs::File, errors::ReadError> {
        let mut file = self
            .file
            .try_clone()
            .await
            .map_err(|_| errors::ReadError::RefFile)?;
        file.seek(SeekFrom::Start(position))
            .await
            .map_err(|_| errors::ReadError::RefFile)?;
        Ok(file)
    }

    /// Gets the directory's bloom filter, initializing it if needed.
    async fn get_bloom_filter(&self) -> Result<&BloomFilter, errors::ReadError> {
        let result = self
            .bloom_filter
            .get_or_try_init(|| async {
                let mut file = self
                    .position_file(self.positions.position_start_bloom_filter as u64)
                    .await?;
                let hashes_bloom_filter_len = file.read_u32_le().await?;

                let bloom_filter_len = self.positions.bloom_filter_len;
                let mut bloom_filter = vec![0; bloom_filter_len];
                let bytes_read = file.read_exact(&mut bloom_filter).await?;

                if bloom_filter.len() != bloom_filter_len {
                    return Err(errors::ReadError::InvalidBloomFilter);
                }

                let filter =
                    BloomFilter::from_u8_array(bloom_filter.as_slice(), hashes_bloom_filter_len);

                Ok(filter)
            })
            .await?;
        Ok(result)
    }

    /// Gets the directory's PHF table, initializing it if needed.
    async fn get_phf_table(&self) -> Result<&HasherState, errors::ReadError> {
        self.phf_table
            .get_or_try_init(|| async {
                let mut file = self
                    .position_file(self.positions.position_start_phf_table as u64)
                    .await?;
                let key = file.read_u64_le().await?;
                let mut disps = vec![(0u16, 0u16); self.positions.phf_disps_len];
                for disp in &mut disps {
                    disp.0 = file.read_u16_le().await?;
                    disp.1 = file.read_u16_le().await?;
                }
                let mut map = vec![0u32; self.positions.phf_entries_len];
                for offset in &mut map {
                    *offset = file.read_u32_le().await?;
                }
                Ok(HasherState { key, disps, map })
            })
            .await
    }

    /// Gets the file offset for an entry using the PHF table.
    async fn get_entry_offset(
        &self,
        name: &[u8],
        phf_table: &HasherState,
    ) -> Result<u32, errors::ReadError> {
        let hashes = hash(name, phf_table.key);
        let buckets_len = calculate_buckets_len(self.num_entries as usize);
        let bucket = (hashes.g as usize) % buckets_len;

        let bucket_offset =
            self.positions.position_start_phf_table + PHF_TABLE_RANDOMIZED_KEY_SIZE + bucket * 4;
        let mut file = self.position_file(bucket_offset as u64).await?;
        let mut bucket_keys = [0u8; 4];
        file.read_exact(&mut bucket_keys).await?;

        let d1 = u16::from_le_bytes(*array_ref!(bucket_keys, 0, 2));
        let d2 = u16::from_le_bytes(*array_ref!(bucket_keys, 2, 2));

        let idx = displace(hashes.f1, hashes.f2, d1 as u32, d2 as u32) as usize
            % self.num_entries as usize;

        let map_offset = self.positions.position_start_phf_table
            + PHF_TABLE_RANDOMIZED_KEY_SIZE
            + buckets_len * 4
            + idx * 4;
        let mut file = self.position_file(map_offset as u64).await?;
        let entry_rel_offset = file.read_u32_le().await?;

        let entry_offset = POSITION_START_HASHES as u32 + entry_rel_offset;

        Ok(entry_offset)
    }

    /// Reads an entry at the given file offset.
    async fn read_entry_at_offset<'a>(
        &'a self,
        name: &'a [u8],
        offset: u32,
    ) -> Result<Option<BorrowedEntry<'a>>, errors::ReadError> {
        let start_entry: u64 =
            self.positions.position_start_entries as u64 + offset as u64 - name.len() as u64 - 3;
        let file = self.position_file(start_entry).await?;
        let mut file = BufReader::new(file);
        let flag = file.read_u8().await;
        check_eof!(flag);
        let flag = flag.unwrap();
        let mut entry_name = Vec::new();
        let entry_to_check = file.read_until(0x00, &mut entry_name).await;
        check_eof!(entry_to_check);
        if &entry_name[..name.len()] != name {
            return Ok(None);
        }
        if flag == B3_DIR_IS_SYM_LINK {
            let mut content = Vec::new();
            file.read_until(0x00, &mut content).await?;
            let content = content[..content.len() - 1].to_vec().into_boxed_slice();
            let static_slice: &'static [u8] = Box::leak(content);
            return Ok(Some(BorrowedEntry {
                name,
                link: BorrowedLink::Path(static_slice),
            }));
        } else {
            let mut buffer: [u8; 32] = vec![0u8; 32].try_into().unwrap();
            file.read_exact(&mut buffer).await?;
            let static_slice: &'static [u8; 32] = unsafe { mem::transmute_copy(&buffer) };
            return Ok(Some(BorrowedEntry {
                name,
                link: BorrowedLink::Content(static_slice),
            }));
        }
        Ok(None)
    }
}

#[cfg(test)]
mod tests {
    use std::env::temp_dir;
    use std::io::Read;
    use std::path::{Path, PathBuf};

    use tokio::io::AsyncWriteExt;
    use tokio_stream::StreamExt;

    use super::*;
    use crate::bucket::dir::writer::DirWriter;
    use crate::bucket::{Bucket, HEADER_DIR_VERSION};
    use crate::entry::OwnedEntry;
    use crate::utils::to_hex;

    async fn create_test_dir(
        temp_dir: &Path,
        entries: Vec<OwnedEntry>,
    ) -> Result<B3Dir, Box<dyn std::error::Error>> {
        let bucket = Bucket::open(&temp_dir).await?;

        let mut writer = DirWriter::new(&bucket, entries.len()).await?;

        for entry in entries {
            writer.insert(&entry).await?;
        }

        // Test commit()
        let root_hash = writer.commit().await?;
        assert_eq!(root_hash.len(), 32);

        let mut file = tokio::fs::File::open(
            temp_dir
                .join("headers")
                .join(to_hex(&root_hash).to_string()),
        )
        .await?;
        let version = file.read_u32_le().await?;
        assert_eq!(version, HEADER_DIR_VERSION);
        let num_entries = file.read_u32_le().await?;

        Ok(B3Dir::new(num_entries, file))
    }

    #[tokio::test]
    async fn test_get_entry() {
        let temp_dir = temp_dir().join("b3fs-test-get-entry");
        let entries = vec![
            OwnedEntry {
                name: "file1".as_bytes().into(),
                link: OwnedLink::Content([1; 32]),
            },
            OwnedEntry {
                name: "file2".as_bytes().into(),
                link: OwnedLink::Content([2; 32]),
            },
        ];
        let mut dir = create_test_dir(&temp_dir, entries).await.unwrap();

        // Test existing file
        let entry = dir.get_entry(b"file1").await.unwrap().unwrap();
        assert_eq!(entry.name, b"file1");
        assert!(matches!(entry.link, BorrowedLink::Content(_)));

        let entry = dir.get_entry(b"file2").await.unwrap().unwrap();
        assert_eq!(entry.name, b"file2");
        assert!(matches!(entry.link, BorrowedLink::Content(_)));

        // Test non-existent entry
        let entry = dir.get_entry(b"nonexistent").await.unwrap();
        assert!(entry.is_none());
        tokio::fs::remove_dir_all(temp_dir).await.unwrap();
    }

    #[tokio::test]
    async fn test_entries_iterator() {
        let temp_dir = temp_dir().join("b3fs-test-entries-iterator");
        let entries = vec![
            OwnedEntry {
                name: "aa_file1".as_bytes().into(),
                link: OwnedLink::Content([1; 32]),
            },
            OwnedEntry {
                name: "aa_file2".as_bytes().into(),
                link: OwnedLink::Content([2; 32]),
            },
            OwnedEntry {
                name: "aa_symlink".as_bytes().into(),
                link: OwnedLink::Link("target".as_bytes().into()),
            },
            OwnedEntry {
                name: "bb_file3".as_bytes().into(),
                link: OwnedLink::Content([2; 32]),
            },
            OwnedEntry {
                name: "bb_symlink2".as_bytes().into(),
                link: OwnedLink::Link("target".as_bytes().into()),
            },
            OwnedEntry {
                name: "bb_symlink3".as_bytes().into(),
                link: OwnedLink::Link("target".as_bytes().into()),
            },
        ];
        let dir = create_test_dir(&temp_dir, entries).await.unwrap();

        let mut iter = dir.entries().await.unwrap();
        let mut count = 0;

        while let Some(entry) = iter.next().await {
            let OwnedEntry { name, link } = entry.unwrap();
            count += 1;
            if name.windows(4).any(|s| s == "file".as_bytes()) {
                assert!(matches!(link, OwnedLink::Content(_)))
            } else {
                assert!(matches!(link, OwnedLink::Link(_)))
            }
        }

        assert_eq!(count, 6);
        tokio::fs::remove_dir_all(temp_dir).await.unwrap();
    }

    #[tokio::test]
    async fn test_hashtree() {
        let temp_dir = temp_dir().join("b3fs-test-hashtree");
        let entries = vec![
            OwnedEntry {
                name: "file1".as_bytes().into(),
                link: OwnedLink::Content([1; 32]),
            },
            OwnedEntry {
                name: "file2".as_bytes().into(),
                link: OwnedLink::Content([2; 32]),
            },
        ];
        let num_entries = entries.len();
        let mut dir = create_test_dir(&temp_dir, entries).await.unwrap();

        let mut hashtree = dir.hashtree().await.unwrap();
        for i in 0..num_entries {
            hashtree.get_hash(i as u32).await.unwrap().unwrap();
        }
        tokio::fs::remove_dir_all(temp_dir).await.unwrap();
    }

    #[tokio::test]
    async fn test_reader_file_with_three_entries() {
        let reader = B3Dir::new(
            3,
            tokio::fs::File::open(
                "tests/fixtures/4bc76dc8621d67c905b214f47cfc68924d7994afd2c181f47a449edbc359b514",
            )
            .await
            .unwrap(),
        );
        let mut entries = reader.entries().await.unwrap();
        let mut count = 0;
        while let Some(en) = entries.next().await {
            count += 1;
            assert!(en.is_ok());
        }
        assert_eq!(count, 3);
    }
}
