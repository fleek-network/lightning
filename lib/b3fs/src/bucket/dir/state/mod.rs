use std::os::unix::fs::FileExt;
use std::path::{Path, PathBuf};

use bytes::{BufMut, BytesMut};
use tokio::fs::{File, OpenOptions};
use tokio::io::{self, AsyncWriteExt, BufWriter};

use super::phf::HasherState;
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
    pub(crate) async fn from_wal_path(path: &Path, num_entries: usize) -> Result<Self, io::Error> {
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
        // Reserve space for bloom filter
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
        let position_start_phf_table = position_start_bloom_filter + num_entries as u64;

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
        bloom_filter: Vec<u8>,
        hash_table: HasherState,
    ) -> Result<(), errors::CommitError> {
        let mut file = self.file.into_inner().try_into_std().unwrap();
        let bytes = hashes.iter().flatten().copied().collect::<Vec<u8>>();
        tokio::task::spawn_blocking(move || async move {
            file.write_at(&bytes, self.position_start_hashes)
                .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
            file.write_at(&bloom_filter, self.position_start_bloom_filter)
                .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
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
