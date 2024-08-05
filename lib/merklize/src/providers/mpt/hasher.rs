use hash256_std_hasher::Hash256StdHasher;

use crate::SimpleHasher;

/// A wrapper around a `[SimpleHasher]` that provides a `[hash_db::Hasher]` implementation.
pub(crate) struct SimpleHasherWrapper<H: SimpleHasher>(H);

impl<H: SimpleHasher> hash_db::Hasher for SimpleHasherWrapper<H>
where
    H: SimpleHasher + Send + Sync,
{
    type Out = [u8; 32];
    type StdHasher = Hash256StdHasher;

    const LENGTH: usize = 32;

    fn hash(data: &[u8]) -> Self::Out {
        H::hash(data)
    }
}
