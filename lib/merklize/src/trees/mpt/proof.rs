use anyhow::Result;
use atomo::SerdeBackend;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use trie_db::proof::verify_proof;

use super::layout::TrieLayoutWrapper;
use crate::{StateKey, StateProof, StateTree};

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct MptStateProof(Vec<Vec<u8>>);

impl MptStateProof {
    pub fn new(proof: Vec<Vec<u8>>) -> Self {
        MptStateProof(proof)
    }
}

impl StateProof for MptStateProof {
    fn verify_membership<K, V, T: StateTree>(
        &self,
        table: impl AsRef<str>,
        key: impl std::borrow::Borrow<K>,
        value: impl std::borrow::Borrow<V>,
        root: crate::StateRootHash,
    ) -> Result<()>
    where
        K: serde::Serialize,
        V: serde::Serialize,
    {
        let state_key = StateKey::new(table, T::Serde::serialize(&key.borrow()));
        let key_hash = state_key.hash::<T::Serde, T::Hasher>();
        let serialized_value = T::Serde::serialize(value.borrow());
        let items = vec![(key_hash.to_vec(), Some(serialized_value))];
        verify_proof::<TrieLayoutWrapper<T::Hasher>, _, _, _>(&root.into(), &self.0, &items)
            .map_err(|e| anyhow::anyhow!(e))
    }

    fn verify_non_membership<K, T: StateTree>(
        self,
        table: impl AsRef<str>,
        key: impl std::borrow::Borrow<K>,
        root: crate::StateRootHash,
    ) -> Result<()>
    where
        K: serde::Serialize,
    {
        let state_key = StateKey::new(table, T::Serde::serialize(&key.borrow()));
        let key_hash = state_key.hash::<T::Serde, T::Hasher>();
        let items: Vec<(Vec<u8>, Option<Vec<u8>>)> = vec![(key_hash.to_vec(), None)];
        verify_proof::<TrieLayoutWrapper<T::Hasher>, _, _, _>(&root.into(), &self.0, &items)
            .map_err(|e| anyhow::anyhow!(e))
    }
}
