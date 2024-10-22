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
use crate::bucket::dir::phf::HasherState;
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
    position_start_hashes: u64,
    position_start_bloom_filter: u64,
    position_start_entries: u64,
}

#[macro_use]
macro_rules! check_eof {
    ($flag:expr) => {
        if let Err(e) = $flag {
            if e.kind() == std::io::ErrorKind::UnexpectedEof {
                break;
            } else {
                return Err(errors::ReadError::from(e));
            }
        }
    };
}

impl B3Dir {
    pub(crate) fn new(num_entries: u32, file: Arc<fs::File>) -> Self {
        debug_assert!(num_entries <= u16::MAX as u32);
        let position_start_hashes = 4 + 4 as u64;
        let position_start_bloom_filter =
            position_start_hashes + (num_entries as usize * 32) as u64;
        let length_bloom_filter = 4 + num_entries as usize;
        let key_phf_table = 8;
        let disps_len = (num_entries as f64 / 5.0).ceil() as usize * 4;
        let entries_len = num_entries as usize * 4;
        let length_phf_table = key_phf_table + disps_len + entries_len;
        let position_start_entries =
            position_start_bloom_filter + length_bloom_filter as u64 + length_phf_table as u64;
        Self {
            num_entries: num_entries as u16,
            file,
            bloom_filter: OnceCell::new(),
            position_start_hashes,
            position_start_bloom_filter,
            position_start_entries,
        }
    }

    pub async fn hashtree(&mut self) -> Result<HashTree<'_>, errors::ReadError> {
        let mut buffer = Vec::with_capacity(self.num_entries as usize * 32);
        let mut file_reader = self.position_file(self.position_start_hashes).await?;
        file_reader.read_exact(&mut buffer).await?;
        let boxed_slice = buffer.into_boxed_slice();
        let static_slice: &'static [u8] = Box::leak(boxed_slice);
        HashTree::try_from(static_slice).map_err(|e| errors::ReadError::InvalidHashtree(e))
    }

    pub async fn get_entry<'a>(
        &'a mut self,
        name: &'a [u8],
    ) -> Result<Option<BorrowedEntry<'a>>, errors::ReadError> {
        let bloom_filter = self.get_bloom_filter().await?;
        if bloom_filter.contains(name) {
            return self.search_entry(name).await;
        }
        Ok(None)
    }

    pub fn entries(&self) -> DirEntriesIter {
        todo!()
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

    async fn search_entry<'a>(
        &'a self,
        name: &'a [u8],
    ) -> Result<Option<BorrowedEntry<'a>>, errors::ReadError> {
        loop {
            let mut file = BufReader::new(self.position_file(self.position_start_entries).await?);
            let flag = file.read_u8().await;
            check_eof!(flag);
            let flag = flag.unwrap();
            let mut entry_name = Vec::new();
            let entry_to_check = file.read_until(0x00, &mut entry_name).await;
            check_eof!(entry_to_check);
            if &entry_name == name {
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
            }
        }
        Ok(None)
    }

    async fn get_bloom_filter(&self) -> Result<&BloomFilter, errors::ReadError> {
        let result = self
            .bloom_filter
            .get_or_try_init(|| async {
                let mut file = self.position_file(self.position_start_bloom_filter).await?;
                let hashes_bloom_filter_len = file.read_u32_le().await?;
                let mut bloom_filter = vec![0; self.num_entries as usize];
                file.read_exact(&mut bloom_filter).await?;
                Ok(BloomFilter::from_u8_array(
                    bloom_filter.as_slice(),
                    hashes_bloom_filter_len,
                )) as Result<BloomFilter, errors::ReadError>
            })
            .await?;
        Ok(result)
    }
}
