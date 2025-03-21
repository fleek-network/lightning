use std::io::{Read, Seek, SeekFrom};
use std::sync::{Arc, RwLock};

use crate::stream::walker::TreeWalker;
use crate::stream::{ProofBuf, ProofEncoder};
use crate::sync::bucket::errors::ReadError;
use crate::sync::bucket::POSITION_START_HASHES;
use crate::utils::{block_counter_from_tree_index, tree_index};

pub struct SyncHashTree<T: Read + Seek + Unpin> {
    file_reader: Arc<RwLock<T>>,
    number_of_blocks: usize,
    current_block: usize,
    pages: Vec<Option<Box<[[u8; 32]]>>>, // Store loaded pages as boxed slices
}

/// An synchronous structure that reads hashes from memory pages.
impl<T> SyncHashTree<T>
where
    T: Read + Seek + Unpin,
{
    pub fn new(file_reader: T, number_of_blocks: usize) -> Self {
        Self {
            file_reader: Arc::new(RwLock::new(file_reader)),
            number_of_blocks,
            current_block: 0,
            pages: vec![None; (number_of_blocks + 1023) / 1024], // Initialize pages
        }
    }

    pub fn get_hash(&mut self, block_number: u32) -> Result<Option<[u8; 32]>, ReadError> {
        self.get_hash_by_index(tree_index(block_number as usize))
    }

    /// Get the hash for the specified block number.
    pub fn get_hash_by_index(&mut self, index: usize) -> Result<Option<[u8; 32]>, ReadError> {
        if index >= self.number_of_blocks * 2 - 1 {
            return Ok(None);
        }

        let block_number = block_counter_from_tree_index(index).unwrap_or(0);
        let page_index = block_number / 1024;
        let offset = index % 1024;

        // Load the page if it is not already loaded
        if self.pages[page_index].is_none() {
            let start_index = POSITION_START_HASHES as u64 + (page_index * 4096) as u64; // 4KB page size
            let file = self.file_reader.clone();
            let mut file_lock = file.write().map_err(|_| ReadError::LockError)?;

            // Determine the remaining bytes in the file
            let file_size = file_lock.seek(SeekFrom::End(0))?; // Get the file size
            file_lock.seek(SeekFrom::Start(start_index))?; // Seek back to the start index

            let bytes_to_read = (file_size - start_index) as usize;
            let mut page_data = vec![0; bytes_to_read.min(4096)]; // Create a buffer with the minimum of remaining bytes or 4096
            file_lock.read_exact(&mut page_data)?;

            let hashes: Vec<[u8; 32]> = page_data
                .chunks_exact(32)
                .map(|slice| {
                    let mut hash = [0; 32];
                    hash.copy_from_slice(slice);
                    hash
                })
                .collect();

            // Store the entire page of hashes
            self.pages[page_index] = Some(hashes.into_boxed_slice()); // Store as boxed slice of
                                                                      // hashes
        }

        // Retrieve the hash from the loaded page
        let hashes = self.pages[page_index].as_ref();
        Ok(hashes.map(|x| x[offset]))
    }

    pub fn generate_proof(&mut self, block_number: u32) -> Result<ProofBuf, ReadError> {
        let tree_len = self.number_of_blocks * 2 - 1;
        let walker = if block_number == 0 {
            TreeWalker::initial(block_number as usize, tree_len)
        } else {
            TreeWalker::proceeding(block_number as usize, tree_len)
        };
        let size = walker.size_hint().0;
        let mut encoder = ProofEncoder::new(size);
        for (direction, index) in walker {
            debug_assert!(index < tree_len, "Index overflow.");
            let hash = self
                .get_hash_by_index(index)?
                .ok_or(ReadError::HashNotFound(index as u32))?;
            encoder.insert(direction, &hash);
        }
        Ok(encoder.finalize())
    }
}
