use std::future::Future;
use std::io;

use lightning_interfaces::types::Blake3Hash;

/// Simple block store interface.
pub trait Store: Send + Clone {
    fn fetch(
        &self,
        location: &str,
        key: &Blake3Hash,
        tag: Option<usize>,
    ) -> impl Future<Output = Option<Block>> + Send;
    fn insert(
        &mut self,
        location: &str,
        key: Blake3Hash,
        block: &[u8],
        tag: Option<usize>,
    ) -> impl Future<Output = io::Result<()>> + Send;
}

pub type Block = Vec<u8>;
