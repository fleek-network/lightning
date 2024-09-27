use serde::{Deserialize, Serialize};

pub mod reader;
mod state;
pub mod uwriter;
pub mod writer;

/// This module contains the implementation of the writer for the B3FS File
///
/// ## Spec:
///
/// - The first 4 bytes is a little endian number for version for `File` version is 0:
///
///     `<version=0: u32 little endian> # 0x01 0x00 0x00 0x00`
///
/// - Then the 2nd 4 bytes is the number of blocks in a file (n):
///
///     `<num_entries: u32 little endian>`
///
/// - we can the calculate the total number of items in the tree (either leaf or internal) using
///   this formula `T = n * 2 - 1` the rest of the file is `T` 32-byte hashes representing the hash
///   tree:
///
///     `<[u8; 32] x T>`
#[derive(Serialize, Deserialize)]
#[repr(C, align(4))]
pub(crate) struct B3FSFile {
    version: u32,
    num_entries: u32,
    hashes: Vec<[u8; 32]>,
}

impl B3FSFile {
    pub fn new(hashes: Vec<[u8; 32]>) -> Self {
        Self {
            version: 1,
            num_entries: hashes.len() as u32,
            hashes,
        }
    }

    pub fn hashes(&self) -> &[[u8; 32]] {
        &self.hashes
    }

    pub fn serialize(&self) -> Vec<u8> {
        bincode::serialize(self).unwrap()
    }

    pub fn deserialize(data: &[u8]) -> Result<Self, bincode::Error> {
        let version: u32 = bincode::deserialize(&data[..4])?;
        let num_entries: u32 = bincode::deserialize(&data[4..8])?;
        let mut hashes = vec![];
        for i in 0..num_entries {
            let range_start = 8 + i as usize * 32_usize;
            let range_end = 8 + (i as usize * 32_usize) + 32_usize;
            let dec: &[u8] = &data[range_start..range_end];
            let mut hash = [0u8; 32];
            hash.copy_from_slice(dec);
            hashes.push(hash);
        }
        Ok(Self {
            version,
            num_entries,
            hashes,
        })
    }
}

#[cfg(test)]
mod test {
    use std::path::PathBuf;

    use rand::random;
    use tokio::fs;

    use crate::bucket::file::B3FSFile;
    use crate::hasher::b3::MAX_BLOCK_SIZE_IN_BYTES;
    use crate::utils;

    pub(super) fn get_random_file() -> Vec<u8> {
        let mut data = Vec::with_capacity(MAX_BLOCK_SIZE_IN_BYTES);
        for _ in 0..8193 {
            let d: [u8; 32] = random();
            data.extend(d);
        }
        data
    }

    pub(super) async fn verify_writer(temp_dir: &PathBuf) {
        let mut dir = std::fs::read_dir(temp_dir);
        assert!(dir.is_ok());
        let dir = dir.unwrap();
        let subdirs = dir.collect::<Result<Vec<_>, _>>().unwrap();

        // Verify wal dir is empty
        let wal_dir = subdirs
            .iter()
            .find(|d| d.path().file_name().unwrap() == "wal");
        assert!(wal_dir.is_some());
        let wal_dir = wal_dir.unwrap().path();
        let wal_dir = std::fs::read_dir(wal_dir).unwrap();
        assert_eq!(0, wal_dir.count());

        // Verify header dir
        let header_dir = subdirs
            .iter()
            .find(|d| d.path().file_name().unwrap() == "headers");
        assert!(header_dir.is_some());
        let header_dir = header_dir.unwrap().path();
        let file_header = std::fs::read_dir(header_dir).unwrap();
        let file_header = file_header.collect::<Result<Vec<_>, _>>().unwrap();
        assert_eq!(1, file_header.len());
        let file_header = file_header[0].path();
        let file_header = std::fs::read(file_header).unwrap();
        let b3fs = B3FSFile::deserialize(&file_header).unwrap();
        assert_eq!(1, b3fs.version);
        assert_eq!(2, b3fs.num_entries);
        assert_eq!(2, b3fs.hashes.len());

        let hashes = b3fs.hashes();

        // Verify block dir
        let block_dir = subdirs
            .iter()
            .find(|d| d.path().file_name().unwrap() == "blocks");
        assert!(block_dir.is_some());
        let block_dir = block_dir.unwrap().path();
        let files = std::fs::read_dir(&block_dir).unwrap();
        let mut files = files.collect::<Result<Vec<_>, _>>().unwrap();
        files.sort_by(|a, b| {
            a.metadata()
                .unwrap()
                .len()
                .cmp(&b.metadata().unwrap().len())
        });
        assert_eq!(2, files.len());
        assert_eq!(32_u64, files[0].metadata().unwrap().len());
        assert_eq!(
            MAX_BLOCK_SIZE_IN_BYTES as u64,
            files[1].metadata().unwrap().len()
        );
        let file_1 = files[0].file_name().to_str().unwrap().to_string();
        let file_2 = files[1].file_name().to_str().unwrap().to_string();
        let hash_1 = utils::to_hex(&hashes[0]).as_str().to_string();
        let hash_2 = utils::to_hex(&hashes[1]).as_str().to_string();
        let hashes_vec = [hash_1, hash_2];
        assert!(hashes_vec.contains(&file_1.to_string()));
        assert!(hashes_vec.contains(&file_2.to_string()));

        fs::remove_dir_all(temp_dir).await.unwrap();
    }
}
