use anyhow::Result;
use atomo::SerdeBackend;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use trie_db::proof::verify_proof;

use super::layout::TrieLayoutWrapper;
use crate::{MerklizeProvider, StateKey, StateProof};

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct MptStateProof(Vec<Vec<u8>>);

impl MptStateProof {
    pub fn new(proof: Vec<Vec<u8>>) -> Self {
        MptStateProof(proof)
    }
}

impl StateProof for MptStateProof {
    fn verify_membership<K, V, M: MerklizeProvider>(
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
        let state_key = StateKey::new(table, M::Serde::serialize(&key.borrow()));
        let key_hash = state_key.hash::<M::Serde, M::Hasher>();
        let serialized_value = M::Serde::serialize(value.borrow());
        let items = vec![(key_hash.to_vec(), Some(serialized_value))];
        verify_proof::<TrieLayoutWrapper<M::Hasher>, _, _, _>(&root.into(), &self.0, &items)
            .map_err(|e| anyhow::anyhow!(e))
    }

    fn verify_non_membership<K, M: crate::MerklizeProvider>(
        self,
        table: impl AsRef<str>,
        key: impl std::borrow::Borrow<K>,
        root: crate::StateRootHash,
    ) -> Result<()>
    where
        K: serde::Serialize,
    {
        let state_key = StateKey::new(table, M::Serde::serialize(&key.borrow()));
        let key_hash = state_key.hash::<M::Serde, M::Hasher>();
        let items: Vec<(Vec<u8>, Option<Vec<u8>>)> = vec![(key_hash.to_vec(), None)];
        verify_proof::<TrieLayoutWrapper<M::Hasher>, _, _, _>(&root.into(), &self.0, &items)
            .map_err(|e| anyhow::anyhow!(e))
    }
}
