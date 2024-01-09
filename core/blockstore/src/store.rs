use std::io;

use lightning_interfaces::types::Blake3Hash;

/// Simple block store interface.
#[trait_variant::make(Store: Send)]
pub trait _Store: Send + Clone {
    async fn fetch(&self, location: &str, key: &Blake3Hash, tag: Option<usize>) -> Option<Block>;
    async fn insert(
        &mut self,
        location: &str,
        key: Blake3Hash,
        block: &[u8],
        tag: Option<usize>,
    ) -> io::Result<()>;
}

pub type Block = Vec<u8>;
