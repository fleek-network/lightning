use std::fs::{File, OpenOptions};
use std::io::{self, BufWriter, Write};
use std::num::NonZeroU32;
use std::os::unix::fs::FileExt;
use std::path::{Path, PathBuf};

use bytes::{BufMut, BytesMut};
use fastbloom_rs::{BloomFilter, FilterBuilder, Hashes, Membership};

use super::phf::{calculate_buckets_len, HasherState, PhfGenerator};
use super::HeaderPositions;
use crate::entry::{BorrowedEntry, BorrowedLink};
use crate::hasher::dir_hasher::B3_DIR_IS_SYM_LINK;
use crate::sync::bucket::dir::POSITION_START_HASHES;
use crate::sync::bucket::{errors, Bucket, HEADER_DIR_VERSION};
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
    positions: HeaderPositions,
}

impl HeaderFile {
    /// Creates a new HeaderFile from a WAL (Write-Ahead Logging) path.
    ///
    /// # Arguments
    ///
    /// * `path` - The path to create the HeaderFile.
    /// * `num_entries` - The number of entries expected in the file.
    /// * `hashes` - The number of hash functions used in the Bloom filter.
    ///
    /// # Returns
    ///
    /// Returns a Result containing the new HeaderFile or an IO error.
    pub(crate) fn from_wal_path(
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
                .open(path.clone())?,
        );
        let positions = HeaderPositions::new(num_entries);
        file.write_all(&HEADER_DIR_VERSION.to_le_bytes())?;
        file.write_all(&(num_entries as u32).to_le_bytes())?;

        // Reserve space for hashes
        file.write_all(&vec![0; positions.length_hashes])?;
        // Reserve space for bloom filter (4 bytes for hashes + 1 byte for each entry) See https://docs.rs/fastbloom-rs/0.5.9/fastbloom_rs/struct.BloomFilter.html#method.from_u8_array
        file.write_all(&hashes.to_le_bytes())?;
        file.write_all(&vec![0; positions.bloom_filter_len])?;
        // Reserve space for PHF table
        // 8 bytes for key
        file.write_all(&0u64.to_le_bytes())?;
        file.write_all(&vec![0; positions.phf_disps_len])?;
        // 4 bytes for each entry
        file.write_all(&vec![0; positions.phf_entries_len])?;

        file.flush()?;

        Ok(Self {
            path,
            file,
            positions,
        })
    }

    /// Commits the HeaderFile by writing all necessary data and moving it to its final location.
    ///
    /// # Arguments
    ///
    /// * `bucket` - The Bucket to which this HeaderFile belongs.
    /// * `root_hash` - The root hash of the directory.
    /// * `hashes` - A vector of hashes for all entries.
    /// * `bloom_filter` - The Bloom filter for quick membership tests.
    /// * `hash_table` - The perfect hash function table.
    ///
    /// # Returns
    ///
    /// Returns a Result indicating success or a CommitError.
    pub(crate) fn commit(
        mut self,
        bucket: &Bucket,
        root_hash: &[u8; 32],
        hashes: Vec<[u8; 32]>,
        bloom_filter: BloomFilter,
        hash_table: HasherState,
    ) -> Result<(), errors::CommitError> {
        let mut file = self.file.into_inner().unwrap();
        let bytes = hashes.iter().flatten().copied().collect::<Vec<u8>>();

        // Write the hashes
        file.write_at(&bytes, POSITION_START_HASHES as u64)
            .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;

        // Write the bloom filter
        file.write_at(
            &bloom_filter.hashes().to_le_bytes(),
            self.positions.position_start_bloom_filter as u64,
        )
        .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
        assert_eq!(
            bloom_filter.get_u8_array().len(),
            self.positions.bloom_filter_len,
            "Number of bloom filter entries does not match with entries in hasher state"
        );
        file.write_at(
            bloom_filter.get_u8_array(),
            self.positions.bloom_filter_start_entries() as u64,
        )
        .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;

        // Write the PHF table
        let disps_len = hash_table.disps.len() * 4;
        let mut disp_bytes = BytesMut::with_capacity(disps_len);

        for disp in hash_table.disps.iter() {
            disp_bytes.put_u16_le(disp.0);
            disp_bytes.put_u16_le(disp.1);
        }
        file.write_at(
            &hash_table.key.to_le_bytes(),
            self.positions.position_start_phf_table as u64,
        )?;
        file.write_at(&disp_bytes, self.positions.phf_table_start_disps() as u64)?;
        let mut map_bytes_buffer = BytesMut::with_capacity(hash_table.map.len() * 4);
        for offset in hash_table.map.iter() {
            map_bytes_buffer.put_u32_le(*offset);
        }
        file.write_at(
            &map_bytes_buffer,
            self.positions.phf_table_start_entries() as u64,
        )?;
        file.flush()?;

        let final_path = bucket.get_header_path(root_hash);
        let header_tmp_file_path = self.path.clone();

        std::fs::rename(header_tmp_file_path, final_path)?;

        Ok(())
    }

    /// Inserts a new entry into the HeaderFile.
    ///
    /// # Arguments
    ///
    /// * `borrowed_entry` - The entry to be inserted.
    ///
    /// # Returns
    ///
    /// Returns a Result indicating success or an InsertError.
    fn insert_entry(
        &mut self,
        borrowed_entry: BorrowedEntry<'_>,
    ) -> Result<usize, errors::InsertError> {
        let (mut flag, content, clen, is_sym) = match borrowed_entry.link {
            crate::entry::BorrowedLink::Content(hash) => (0, hash.as_slice(), 32, false),
            crate::entry::BorrowedLink::Path(link) => {
                (B3_DIR_IS_SYM_LINK, link, link.len() + 1, true)
            },
        };

        let mut buffer = Vec::with_capacity(4 + borrowed_entry.name.len() + clen);
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
        self.file.write_all(&buffer)?;
        self.file.flush()?;
        Ok(buffer.len())
    }
}

/// Represents the internal state of a directory.
pub(super) struct InnerDirState<T> {
    bucket: Bucket,
    phf_generator: PhfGenerator,
    next_position: u32,
    temp_file_path: PathBuf,
    header_file: HeaderFile,
    bloom_filter: BloomFilter,
    pub collector: T,
}

/// Trait for types that can collect and process directory entries.
pub trait WithCollector {
    /// Processes a new entry being inserted into the directory.
    fn on_insert(
        &mut self,
        borrowed_entry: BorrowedEntry<'_>,
        last_entry: bool,
    ) -> Result<(), errors::InsertError>;

    /// Finalizes the collection process and returns the root hash and hash tree.
    fn on_commit(self) -> Result<([u8; 32], Vec<[u8; 32]>), errors::CommitError>;
}

/// Trait defining the operations that can be performed on a directory state.
pub trait DirState {
    /// Inserts a new entry into the directory.
    fn insert_entry(
        &mut self,
        borrowed_entry: BorrowedEntry<'_>,
        last_entry: bool,
    ) -> Result<(), errors::InsertError>;

    /// Commits the changes made to the directory.
    fn commit(self) -> Result<[u8; 32], errors::CommitError>;

    /// Rolls back any changes made to the directory.
    fn rollback(self) -> Result<(), io::Error>;
}

impl<T: WithCollector> DirState for InnerDirState<T> {
    fn insert_entry(
        &mut self,
        borrowed_entry: BorrowedEntry<'_>,
        last_entry: bool,
    ) -> Result<(), errors::InsertError> {
        let i = self.next_position;
        self.phf_generator.push(borrowed_entry.name, i);
        let len_inserted = self.header_file.insert_entry(borrowed_entry)?;
        self.next_position += len_inserted as u32;
        self.bloom_filter.add(borrowed_entry.name);
        self.collector.on_insert(borrowed_entry, last_entry)?;
        Ok(())
    }

    fn commit(mut self) -> Result<[u8; 32], errors::CommitError> {
        let (root_hash, tree) = self.collector.on_commit()?;
        let hash_tree = self.phf_generator.finalize();
        self.header_file
            .commit(&self.bucket, &root_hash, tree, self.bloom_filter, hash_tree)?;

        Ok(root_hash)
    }

    fn rollback(self) -> Result<(), io::Error> {
        std::fs::remove_dir_all(self.temp_file_path)
    }
}

impl<T: WithCollector + Default> InnerDirState<T> {
    /// Creates a new InnerDirState with a custom collector.
    ///
    /// # Arguments
    ///
    /// * `bucket` - The Bucket to which this directory belongs.
    /// * `num_entries` - The expected number of entries in the directory.
    /// * `collector` - The custom collector to use.
    ///
    /// # Returns
    ///
    /// Returns a Result containing the new InnerDirState or an IO error.
    pub(crate) fn new_with_collector(
        bucket: &Bucket,
        num_entries: usize,
        collector: T,
    ) -> Result<Self, io::Error> {
        let phf_generator = PhfGenerator::new(num_entries);
        let temp_file_path = bucket.get_new_wal_path();
        std::fs::create_dir_all(&temp_file_path)?;
        let bloom_filter = FilterBuilder::new(num_entries as u64, 0.01).build_bloom_filter();
        let header_file =
            HeaderFile::from_wal_path(&temp_file_path, num_entries, bloom_filter.hashes())?;
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

    /// Creates a new InnerDirState with a default collector.
    ///
    /// # Arguments
    ///
    /// * `bucket` - The Bucket to which this directory belongs.
    /// * `num_entries` - The expected number of entries in the directory.
    ///
    /// # Returns
    ///
    /// Returns a Result containing the new InnerDirState or an IO error.
    pub(crate) fn new(bucket: &Bucket, num_entries: usize) -> Result<Self, io::Error> {
        Self::new_with_collector(bucket, num_entries, T::default())
    }
}
