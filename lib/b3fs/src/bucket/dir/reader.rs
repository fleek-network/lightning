use std::borrow::Borrow;
use std::cell::Cell;
use std::io::SeekFrom;
use std::mem;
use std::ops::Deref;

use fastbloom_rs::{BloomFilter, Membership};
use tokio::fs::{self};
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncSeekExt, BufReader};
use tokio::sync::OnceCell;
use triomphe::Arc;

use super::iter::DirEntriesIter;
use crate::bucket::dir::phf::{calculate_buckets_len, displace, hash, HasherState};
use crate::bucket::dir::HeaderPositions;
use crate::bucket::errors;
use crate::collections::HashTree;
use crate::entry::{BorrowedEntry, BorrowedLink, OwnedLink};
use crate::hasher::dir_hasher::B3_DIR_IS_SYM_LINK;

pub struct B3Dir {
    /// The number of entries in this directory.
    num_entries: u16,
    /// The actual header file.
    file: Arc<fs::File>,
    bloom_filter: OnceCell<BloomFilter>,
    hashtree: OnceCell<HashTree<'static>>,
    positions: HeaderPositions,
    phf_table: OnceCell<HasherState>,
}

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
    pub(crate) fn new(num_entries: u32, file: Arc<fs::File>) -> Self {
        debug_assert!(num_entries <= u16::MAX as u32);
        let positions = HeaderPositions::new(num_entries as usize);
        Self {
            num_entries: num_entries as u16,
            file,
            bloom_filter: OnceCell::new(),
            hashtree: OnceCell::new(),
            phf_table: OnceCell::new(),
            positions,
        }
    }

    pub async fn hashtree(&mut self) -> Result<&HashTree<'static>, errors::ReadError> {
        self.hashtree
            .get_or_try_init(|| async {
                let mut buffer = Vec::with_capacity(self.num_entries as usize * 32);
                let mut file_reader = self
                    .position_file(self.positions.position_start_hashes as u64)
                    .await?;
                file_reader.read_exact(&mut buffer).await?;
                let boxed_slice = buffer.into_boxed_slice();
                let static_slice: &'static [u8] = Box::leak(boxed_slice);
                HashTree::try_from(static_slice).map_err(|e| errors::ReadError::InvalidHashtree(e))
            })
            .await
    }

    pub async fn get_entry<'a>(
        &'a mut self,
        name: &'a [u8],
    ) -> Result<Option<BorrowedEntry<'a>>, errors::ReadError> {
        let bloom_filter = self.get_bloom_filter().await?;
        if bloom_filter.contains(name) {
            let phf_table = self.get_phf_table().await?;
            let entry_offset = self.get_entry_offset(name, phf_table)?;
            return self.read_entry_at_offset(name, entry_offset).await;
        }
        Ok(None)
    }

    pub async fn entries<'a>(&'a self) -> Result<DirEntriesIter<'a>, errors::ReadError> {
        let result = DirEntriesIter::new(
            self.file.try_clone().await?,
            self.positions.position_start_entries as u64,
        )
        .await?;
        Ok(result)
    }

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

    fn get_entry_offset(
        &self,
        name: &[u8],
        phf_table: &HasherState,
    ) -> Result<u32, errors::ReadError> {
        let hashes = hash(name, phf_table.key);
        let bucket = (hashes.g as usize) % phf_table.disps.len();
        let (d1, d2) = phf_table.disps[bucket];
        let idx =
            displace(hashes.f1, hashes.f2, d1 as u32, d2 as u32) as usize % phf_table.map.len();
        Ok(phf_table.map[idx])
    }

    async fn read_entry_at_offset<'a>(
        &'a self,
        name: &'a [u8],
        offset: u32,
    ) -> Result<Option<BorrowedEntry<'a>>, errors::ReadError> {
        let file = self
            .position_file(self.positions.position_start_entries as u64 + offset as u64)
            .await?;
        let mut file = BufReader::new(file);
        let flag = file.read_u8().await;
        check_eof!(flag);
        let flag = flag.unwrap();
        let mut entry_name = Vec::new();
        let entry_to_check = file.read_until(0x00, &mut entry_name).await;
        check_eof!(entry_to_check);
        if &entry_name != name {
            return Err(errors::ReadError::InvalidEntryAtOffset(
                name.to_vec(),
                entry_name.to_vec(),
                offset,
            ));
        }
        if flag == B3_DIR_IS_SYM_LINK {
            let mut content = Vec::new();
            file.read_until(0x00, &mut content).await?;
            let content = content.into_boxed_slice();
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
    use std::path::{Path, PathBuf};

    use tokio::io::AsyncWriteExt;
    use tokio_stream::StreamExt;

    use super::*;
    use crate::bucket::dir::writer::DirWriter;
    use crate::bucket::Bucket;
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
        let num_entries = file.read_u32_le().await?;
        let file = Arc::new(file);

        Ok(B3Dir::new(num_entries, file))
    }

    #[tokio::test]
    async fn test_get_entry() {
        let temp_dir = temp_dir().join("b3fs");
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
        let temp_dir = temp_dir().join("b3fs");
        let entries = vec![
            OwnedEntry {
                name: "file1".as_bytes().into(),
                link: OwnedLink::Content([1; 32]),
            },
            OwnedEntry {
                name: "file2".as_bytes().into(),
                link: OwnedLink::Content([2; 32]),
            },
            OwnedEntry {
                name: "symlink".as_bytes().into(),
                link: OwnedLink::Link("target".as_bytes().into()),
            },
        ];
        let dir = create_test_dir(&temp_dir, entries).await.unwrap();

        let mut iter = dir.entries().await.unwrap();
        let mut count = 0;

        while let Some(entry) = iter.next().await {
            let entry = entry.unwrap();
            count += 1;
            match entry.name {
                b"file1\0" | b"file2\0" => assert!(matches!(entry.link, BorrowedLink::Content(_))),
                b"symlink" => assert!(matches!(entry.link, BorrowedLink::Path(_))),
                _ => panic!(
                    "Unexpected entry: {:?}",
                    String::from_utf8(entry.name.to_vec()).unwrap()
                ),
            }
        }

        assert_eq!(count, 3);
        tokio::fs::remove_dir_all(temp_dir).await.unwrap();
    }

    #[tokio::test]
    async fn test_hashtree() {
        let temp_dir = temp_dir().join("b3fs");
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

        let hashtree = dir.hashtree().await.unwrap();
        assert_eq!(hashtree.len(), 2);
        tokio::fs::remove_dir_all(temp_dir).await.unwrap();
    }
}
