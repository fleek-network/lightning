use std::io;

use async_trait::async_trait;
use lightning_interfaces::Blake3Hash;

use crate::{Block, Key};

/// Simple block store interface.
#[async_trait]
pub trait Store: Send + Clone {
    async fn fetch(&self, key: &Blake3Hash, tag: Option<usize>) -> Option<Block>;
    async fn insert(&mut self, key: Blake3Hash, block: Block, tag: Option<usize>)
    -> io::Result<()>;
}
