pub mod iter;
pub mod reader;
mod state;
pub mod uwriter;
pub mod writer;

// Only exposed for benchmarks.
#[doc(hidden)]
pub mod phf;

#[cfg(test)]
mod tests {
    use std::env::temp_dir;

    use super::*;
    use crate::bucket::Bucket;
    use crate::hasher::dir_hasher::DirectoryHasher;
    use crate::test_utils::*;

    pub(super) async fn setup_bucket() -> Result<Bucket, Box<dyn std::error::Error>> {
        let temp_dir = temp_dir().join("b3fs_tests");
        tokio::fs::create_dir_all(&temp_dir).await?;
        Ok(Bucket::open(&temp_dir).await?)
    }
}
