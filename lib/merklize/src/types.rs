use atomo::SerdeBackend;
use serde::{Deserialize, Serialize};

use crate::{SimpleHash, SimpleHasher};

/// State root hash of the merkle tree.
pub type StateRootHash = SimpleHash;

/// Hash of a leaf value key in the state tree. This is not the same as a tree node key, but rather
/// a value in the dataset (leaf nodes) and the key that's used to look it up in the state.
pub type StateKeyHash = SimpleHash;

/// Encapsulation of a value (leaf node) key in the state tree, including the state table name and
/// entry key.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StateKey {
    pub table: String,
    pub key: Vec<u8>,
}

impl StateKey {
    /// Create a new `StateKey` with the given table name and key.
    pub fn new(table: impl AsRef<str>, key: Vec<u8>) -> Self {
        Self {
            table: table.as_ref().to_string(),
            key,
        }
    }

    /// Build and return a hash for the state key.
    pub fn hash<S: SerdeBackend, H: SimpleHasher>(&self) -> StateKeyHash {
        StateKeyHash::build::<H>(S::serialize(&self))
    }
}
