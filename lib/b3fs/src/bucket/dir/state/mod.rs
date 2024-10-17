use std::num::NonZeroU32;
use std::os::unix::fs::FileExt;
use std::path::{Path, PathBuf};

use bytes::{BufMut, BytesMut};
use fastbloom_rs::{BloomFilter, FilterBuilder, Hashes, Membership};
use tokio::fs::{File, OpenOptions};
use tokio::io::{self, AsyncWriteExt, BufWriter};

use super::phf::{HasherState, PhfGenerator};
use crate::bucket::{errors, Bucket};
use crate::entry::{BorrowedEntry, BorrowedLink};
use crate::hasher::dir_hasher::B3_DIR_IS_SYM_LINK;
use crate::utils::random_file_from;

pub(super) mod uwriter;
pub(super) mod writer;

/// A block file is a file that contains the data of a block.
///
/// ```text
/// HEADER_DIR = VERSION-NUM_ENTRIES-VEC [HASHES] [BLOOM_FILTER based on the entries' names] PHF-TABLE [ENTRIES]
/// [ENTRIES] = [NULL TERMINATED STRING: entry name] [IS_SYM_LINK] [ENTRY CONTENT]
///                         ;;^--- If a file/dir (not symlink), it's a 32byte hash
///                         ;;     otherwise, just a null terminated string for symlink
/// PHF-TABLE = u64: key, [(u16, u16]]: disps, [u32]: offset
/// ```
pub struct HeaderFile {
    path: PathBuf,
    file: BufWriter<File>,
    position_start_hashes: u64,
    position_start_bloom_filter: u64,
    position_start_phf_table: u64,
}

impl HeaderFile {
    pub(crate) async fn from_wal_path(
        path: &Path,
        num_entries: usize,
        hashes: u32,
    ) -> Result<Self, io::Error> {
        let path = random_file_from(path);
        let mut file = BufWriter::new(
            OpenOptions::new()
                .create(true)
                .truncate(true)
                .write(true)
                .open(path.clone())
                .await?,
        );
        let version: u32 = 1;
        file.write_u32_le(version).await?;
        file.write_u32_le(num_entries as u32).await?;
        // Reserve space for hashes
        file.write_all(&vec![0; num_entries * 32]).await?;
        // Reserve space for bloom filter (4 bytes for hashes + 1 byte for each entry) See https://docs.rs/fastbloom-rs/0.5.9/fastbloom_rs/struct.BloomFilter.html#method.from_u8_array
        file.write_u32_le(hashes).await?;
        file.write_all(&vec![0; num_entries]).await?;
        // Reserve space for PHF table
        // 8 bytes for key
        file.write_u64_le(0).await?;
        // entries.len() + 5 -1 / 5
        let disps_len = (num_entries as f64 / 5.0).ceil() as usize;
        file.write_all(&vec![0; disps_len * 4]).await?;
        // 4 bytes for each entry
        file.write_all(&vec![0; num_entries * 4]).await?;

        let position_start_hashes = 8;
        let position_start_bloom_filter = position_start_hashes + (num_entries * 32) as u64;
        let position_start_phf_table = position_start_bloom_filter + num_entries as u64 + 4;
        Ok(Self {
            path,
            file,
            position_start_hashes,
            position_start_bloom_filter,
            position_start_phf_table,
        })
    }

    pub(crate) async fn commit(
        mut self,
        bucket: &Bucket,
        root_hash: &[u8; 32],
        hashes: Vec<[u8; 32]>,
        bloom_filter: BloomFilter,
        hash_table: HasherState,
    ) -> Result<(), errors::CommitError> {
        let mut file = self.file.into_inner().try_into_std().unwrap();
        let bytes = hashes.iter().flatten().copied().collect::<Vec<u8>>();
        tokio::task::spawn_blocking(move || async move {
            // Write the hashes
            file.write_at(&bytes, self.position_start_hashes)
                .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;

            // Write the bloom filter
            file.write_at(
                &bloom_filter.hashes().to_le_bytes(),
                self.position_start_bloom_filter,
            )
            .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
            file.write_at(
                bloom_filter.get_u8_array(),
                self.position_start_bloom_filter + 4,
            )
            .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;

            // Write the PHF table
            let hash_table = hash_table;
            let disp_bytes = bincode::serialize(&hash_table.disps)
                .map_err(|e| errors::CommitError::Serialization(e.to_string()))?;
            let map_bytes = bincode::serialize(&hash_table.map)
                .map_err(|e| errors::CommitError::Serialization(e.to_string()))?;
            file.write_at(&hash_table.key.to_le_bytes(), self.position_start_phf_table)?;
            file.write_at(&disp_bytes, self.position_start_phf_table + 8)?;
            file.write_at(
                &map_bytes,
                self.position_start_phf_table + 8 + disp_bytes.len() as u64,
            )?;
            Ok(()) as Result<(), errors::CommitError>
        })
        .await?
        .await?;

        let final_path = bucket.get_header_path(root_hash);
        let header_tmp_file_path = self.path.clone();

        tokio::fs::rename(header_tmp_file_path, final_path).await?;

        Ok(())
    }

    async fn insert_entry<'a>(
        &mut self,
        borrowed_entry: BorrowedEntry<'a>,
    ) -> Result<(), errors::InsertError> {
        let (mut flag, content, clen, is_sym) = match borrowed_entry.link {
            crate::entry::BorrowedLink::Content(hash) => (0, hash.as_slice(), 32, false),
            crate::entry::BorrowedLink::Path(link) => {
                (B3_DIR_IS_SYM_LINK, link, link.len() + 1, true)
            },
        };

        let mut buffer = BytesMut::with_capacity(4 + borrowed_entry.name.len() + clen);
        buffer.put_u8(flag);
        buffer.extend_from_slice(borrowed_entry.name);

        // A valid file name never contains the "null" byte. So we can use it as terminator
        // instead of prefixing the length.
        buffer.put_u8(0x00);

        // Fill the rest of the bytes with the content link.
        buffer.extend_from_slice(content);
        if is_sym {
            buffer.put_u8(0x00);
        }
        self.file.write_all(&buffer).await?;

        Ok(())
    }
}

pub(super) struct InnerDirState<T> {
    bucket: Bucket,
    phf_generator: PhfGenerator,
    next_position: u32,
    temp_file_path: PathBuf,
    header_file: HeaderFile,
    bloom_filter: BloomFilter,
    pub collector: T,
}

pub trait WithCollector {
    async fn on_insert(
        &mut self,
        borrowed_entry: BorrowedEntry<'_>,
    ) -> Result<(), errors::InsertError>;
    async fn on_commit(self) -> Result<([u8; 32], Vec<[u8; 32]>), errors::CommitError>;
}

pub trait DirState {
    async fn insert_entry<'a>(
        &mut self,
        borrowed_entry: BorrowedEntry<'a>,
    ) -> Result<(), errors::InsertError>;
    async fn commit(self) -> Result<[u8; 32], errors::CommitError>;
    async fn rollback(self) -> Result<(), io::Error>;
}

impl<T: WithCollector> DirState for InnerDirState<T> {
    async fn insert_entry<'b>(
        &mut self,
        borrowed_entry: BorrowedEntry<'b>,
    ) -> Result<(), errors::InsertError> {
        let mut bytes = match borrowed_entry.link {
            BorrowedLink::Content(content) => content,
            BorrowedLink::Path(path) => path,
        };
        let i = self.next_position;
        let pos = unsafe { NonZeroU32::new_unchecked(i + 1) };
        self.phf_generator.push(bytes, pos);
        self.next_position += borrowed_entry.name.len() as u32;
        self.header_file.insert_entry(borrowed_entry).await?;
        self.bloom_filter.add(borrowed_entry.name);
        self.collector.on_insert(borrowed_entry).await?;
        Ok(())
    }

    async fn commit(mut self) -> Result<[u8; 32], errors::CommitError> {
        let (root_hash, tree) = self.collector.on_commit().await?;
        let hash_tree = self.phf_generator.finalize();
        self.header_file
            .commit(&self.bucket, &root_hash, tree, self.bloom_filter, hash_tree)
            .await?;

        Ok(root_hash)
    }

    async fn rollback(self) -> Result<(), io::Error> {
        tokio::fs::remove_dir_all(self.temp_file_path).await
    }
}

impl<T: WithCollector + Default> InnerDirState<T> {
    pub(crate) async fn new_with_collector(
        bucket: &Bucket,
        num_entries: usize,
        collector: T,
    ) -> Result<Self, io::Error> {
        let phf_generator = PhfGenerator::new(num_entries);
        let temp_file_path = bucket.get_new_wal_path();
        tokio::fs::create_dir_all(&temp_file_path).await?;
        let bloom_filter = FilterBuilder::new(num_entries as u64, 0.01).build_bloom_filter();
        let header_file =
            HeaderFile::from_wal_path(&temp_file_path, num_entries, bloom_filter.hashes()).await?;
        Ok(Self {
            bucket: bucket.clone(),
            phf_generator,
            next_position: 0,
            temp_file_path,
            header_file,
            bloom_filter,
            collector,
        })
    }

    pub(crate) async fn new(bucket: &Bucket, num_entries: usize) -> Result<Self, io::Error> {
        Self::new_with_collector(bucket, num_entries, T::default()).await
    }
}
