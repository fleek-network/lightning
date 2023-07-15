use async_trait::async_trait;

use crate::{Block, Key};

/// Simple block store interface.
#[async_trait]
pub trait Store {
    async fn fetch(&self, key: &Key) -> Option<Block>;
    async fn insert(&mut self, key: Key, block: Block);
}
