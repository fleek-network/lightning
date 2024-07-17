use std::path::PathBuf;

use bytes::Bytes;

use crate::collections::HashTree;

const BLOCK_STZE: u32 = 2 << 18; // 256KB

pub struct FilePutter {
    /// The current active block which we are writing to.
    current_block: MutBlock,
    /// The blocks that we have finished writing to.
    written_blocks: Vec<TempBlock>,
}

/// An active mutable block that can still be written to.
struct MutBlock {
    // max=256KB -> 2**18-1 -> x<2**32-1
    written: u32,
    file: tokio::fs::File,
}

/// A finished block in the temp directory. On finalization this block will be renamed to be the
/// given hash and stored in `/BLOCKS` directory. Otherwise on drop the file from `/TEMP` will be
/// removed.
struct TempBlock {
    path: Option<PathBuf>,
}

impl FilePutter {
    pub fn write(&mut self, mut content: Bytes) {
        while !content.is_empty() {
            let take_bytes = BLOCK_STZE - self.current_block.written;
            if take_bytes == 0 {
                // There is no more space in the current MutBlock but we still have content to write
                // into the next blocks.
                let block = self.current_block.flush();
                self.written_blocks.push(block);
            }

            let bytes = content.split_to(take_bytes as usize);
            self.current_block.write(bytes);
        }
    }

    /// Finalize this
    pub fn finalize(mut self, tree: HashTree) {
        if let Some(block) = self.current_block.finalize() {
            self.written_blocks.push(block);
        }

        assert_eq!(
            tree.len(),
            self.written_blocks.len(),
            "The provided hash tree mismatches the sz"
        );
    }
}

impl MutBlock {
    pub fn write(&mut self, bytes: Bytes) {
        self.written += bytes.len() as u32;
        assert!(
            self.written <= BLOCK_STZE,
            "The block should not exceed the maximum block size."
        );
    }

    pub fn flush(&mut self) -> TempBlock {
        self.written = 0;
        todo!()
    }

    pub fn finalize(self) -> Option<TempBlock> {
        todo!()
    }
}

impl TempBlock {
    /// Finalize this impending block
    pub fn finalize(mut self, hash: &[u8; 32]) {
        // todo(qti3e):
        // rename(self.path.take(), gen_block_path(hash))
    }
}

impl Drop for TempBlock {
    fn drop(&mut self) {
        todo!()
    }
}
