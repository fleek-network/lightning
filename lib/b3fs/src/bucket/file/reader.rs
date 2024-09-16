use tokio::fs::{self};
use triomphe::Arc;

use crate::collections::HashTree;

pub struct B3File {
    num_blocks: u32,
    file: Arc<fs::File>,
}

impl B3File {
    pub(crate) fn new(num_blocks: u32, file: Arc<fs::File>) -> Self {
        Self { num_blocks, file }
    }

    pub fn hashtree(&self) -> HashTree {
        todo!()
    }
}
