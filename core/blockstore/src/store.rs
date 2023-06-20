use crate::{Block, Key};

/// Basic get and put store trait.
pub trait Store {
    fn store_get(&self, key: &Key) -> Option<Block>;
    fn store_put(&mut self, key: Key, block: Block);
}
