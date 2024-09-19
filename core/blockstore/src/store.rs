use std::future::Future;
use std::io;

use b3fs::bucket::ContentHeader;
use lightning_interfaces::types::Blake3Hash;

/// Simple block store interface.
pub trait Store: Send + Clone {
    fn fetch(
        &self,
        key: &Blake3Hash,
        tag: Option<usize>,
    ) -> impl Future<Output = Option<ContentHeader>> + Send;
    fn insert(
        &mut self,
        location: &Blake3Hash,
        key: Blake3Hash,
        block: &[u8],
        tag: Option<usize>,
    ) -> impl Future<Output = io::Result<()>> + Send;
}

pub type Block = Vec<u8>;
