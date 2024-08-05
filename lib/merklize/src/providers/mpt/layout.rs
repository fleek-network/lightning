use std::marker::PhantomData;

use reference_trie::ReferenceNodeCodecNoExtMeta;
use trie_db::TrieLayout;

use super::hasher::SimpleHasherWrapper;
use crate::SimpleHasher;

/// A wrapper around a `[trie_db::TrieLayout]` that uses a given `[SimpleHasher]` as the layout
/// hasher.
pub(crate) struct TrieLayoutWrapper<H: SimpleHasher> {
    _phantom: PhantomData<H>,
}

impl<H> TrieLayout for TrieLayoutWrapper<H>
where
    H: SimpleHasher + Send + Sync,
{
    const USE_EXTENSION: bool = false;
    const ALLOW_EMPTY: bool = false;
    const MAX_INLINE_VALUE: Option<u32> = None;

    type Hash = SimpleHasherWrapper<H>;
    type Codec = ReferenceNodeCodecNoExtMeta<SimpleHasherWrapper<H>>;
}
