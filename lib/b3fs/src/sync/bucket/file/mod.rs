use arrayref::array_ref;
use serde::{Deserialize, Serialize};

use super::errors::ReadError;
use crate::collections::HashTree;
use crate::sync::bucket::POSITION_START_HASHES;

pub mod reader;
pub(crate) mod state;
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
pub struct B3FSFile {
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

    pub fn deserialize(data: &[u8]) -> Self {
        // Skip type
        let version: u32 = u32::from_le_bytes(*array_ref!(data, 0, 4));
        let num_entries: u32 = u32::from_le_bytes(*array_ref!(data, 4, 4));
        let num_hashes = (2 * num_entries) - 1;
        assert!(
            data.len() == POSITION_START_HASHES + num_hashes as usize * 32,
            "Invalid data length {} - num_entries: {} - num_hashes: {}",
            data.len(),
            num_entries,
            num_hashes
        );
        let mut hashes = vec![];
        for i in 0..num_hashes {
            let range_start = POSITION_START_HASHES + i as usize * 32_usize;
            let range_end = range_start + 32_usize;
            let dec: &[u8] = &data[range_start..range_end];
            let mut hash = [0u8; 32];
            hash.copy_from_slice(dec);
            hashes.push(hash);
        }
        Self {
            version,
            num_entries,
            hashes,
        }
    }

    pub fn get_hash_tree(&self) -> Result<HashTree, ReadError> {
        HashTree::try_from(self.hashes()).map_err(|_| ReadError::HashTreeConversion)
    }
}

#[cfg(test)]
mod tests {
    use std::char::MAX;
    use std::env::temp_dir;
    use std::io::{Read, Seek};
    use std::path::PathBuf;

    use rand::random;

    use crate::hasher::b3::MAX_BLOCK_SIZE_IN_BYTES;
    use crate::sync::bucket::file::reader::B3File;
    use crate::sync::bucket::file::writer::FileWriter;
    use crate::sync::bucket::file::B3FSFile;
    use crate::sync::bucket::tests::get_random_file;
    use crate::sync::bucket::{
        Bucket,
        HEADER_FILE_VERSION,
        POSITION_START_HASHES,
        POSITION_START_NUM_ENTRIES,
    };
    use crate::utils;

    pub(super) fn verify_writer(temp_dir: &PathBuf, n: usize) {
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
        let b3fs = B3FSFile::deserialize(&file_header);
        let num_hashes = 2 * n - 1;
        assert_eq!(HEADER_FILE_VERSION, b3fs.version);
        assert_eq!(n, b3fs.num_entries as usize);
        assert_eq!(num_hashes, b3fs.hashes.len());

        let hashes = b3fs.hashes();

        // Verify block dir
        let block_dir = subdirs
            .iter()
            .find(|d| d.path().file_name().unwrap() == "blocks");
        assert!(block_dir.is_some());
        let block_dir = block_dir.unwrap().path();
        let files = std::fs::read_dir(&block_dir).unwrap();
        let files = files.collect::<Result<Vec<_>, _>>().unwrap();
        assert_eq!(n, files.len());
        let hashes = hashes
            .iter()
            .map(|h| utils::to_hex(h).as_str().to_string())
            .collect::<Vec<String>>();
        for file in files.iter() {
            assert!(file.metadata().unwrap().len() <= MAX_BLOCK_SIZE_IN_BYTES as u64);
            let file_name = file.file_name().to_str().unwrap().to_string();
            assert!(hashes.contains(&file_name));
        }

        std::fs::remove_dir_all(temp_dir).unwrap();
    }

    #[test]
    fn test_trusted_writer_follow_reader() {
        let temp_dir_name = random::<[u8; 32]>();
        let temp_dir = temp_dir().join(format!(
            "test_writer_follow_reader_{}",
            utils::to_hex(&temp_dir_name)
        ));
        let bucket = Bucket::open(&temp_dir).unwrap();
        let mut writer = FileWriter::new(&bucket).unwrap();
        let data = get_random_file(8192 * 4);
        writer.write(&data).unwrap();
        writer.commit().unwrap();

        let binding = temp_dir.join("blocks");
        let blocks_dir = binding.as_path();
        let file_blocks = std::fs::read_dir(blocks_dir).unwrap();
        let file_blocks = file_blocks
            .map(|d| d.map(|x| x.file_name().into_string().unwrap()))
            .collect::<Result<Vec<String>, _>>()
            .unwrap();

        let binding = temp_dir.join("headers");
        let header_dir = binding.as_path();
        let file_header = std::fs::read_dir(header_dir).unwrap();
        let file_header = file_header.collect::<Result<Vec<_>, _>>().unwrap();
        assert_eq!(1, file_header.len());
        let file_header = file_header[0].path();

        let mut file = std::fs::File::open(file_header).unwrap();
        file.seek(std::io::SeekFrom::Start(POSITION_START_NUM_ENTRIES as u64))
            .unwrap();

        let mut buf = [0; 4];
        file.read_exact(&mut buf).unwrap();
        let num_entries = u32::from_le_bytes(buf);

        let mut reader = B3File::new(num_entries, file);
        let mut hashtree = reader.hashtree().unwrap();
        let mut counter = 0;
        while let Ok(Some(block)) = hashtree.get_hash(counter) {
            counter += 1;
            assert!(
                file_blocks.contains(&utils::to_hex(&block).as_str().to_string()),
                "Block not found: {} - Blocks: {:?}",
                utils::to_hex(&block),
                file_blocks
            );
        }
        assert_eq!(counter, num_entries);
        std::fs::remove_dir_all(&temp_dir).unwrap();
    }
}
