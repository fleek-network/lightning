use std::marker::PhantomData;
use std::num::NonZeroUsize;
use std::sync::{Arc, Mutex};

use anyhow::Result;
use atomo::{SerdeBackend, StorageBackend, TableSelector};
use jmt::storage::{HasPreimage, LeafNode, Node, NodeKey, TreeReader};
use jmt::{KeyHash, OwnedValue, Version};
use lru::LruCache;
use tracing::trace;

use super::provider::{SharedKeysTableRef, SharedNodesTableRef};
use crate::StateKey;

/// A `[jmt::TreeReader]` and `[jmt::HasPreImage]` implementation that uses a given table selector
/// execution context to read from the database. This acts as an adapter between a merklize provider
/// functionality and the implementation using `[jmt::JellyfishMerkleTree]`.
pub(crate) struct Adapter<'a, B: StorageBackend, S: SerdeBackend> {
    ctx: &'a TableSelector<B, S>,
    nodes_table: SharedNodesTableRef<'a, B, S>,
    keys_table: SharedKeysTableRef<'a, B, S>,
    keys_cache: Arc<Mutex<LruCache<KeyHash, StateKey>>>,
    _phantom: PhantomData<()>,
}

impl<'a, B: StorageBackend, S: SerdeBackend> Adapter<'a, B, S> {
    pub fn new(
        ctx: &'a TableSelector<B, S>,
        nodes_table: SharedNodesTableRef<'a, B, S>,
        keys_table: SharedKeysTableRef<'a, B, S>,
    ) -> Self {
        Self {
            ctx,
            nodes_table,
            keys_table,
            keys_cache: Arc::new(Mutex::new(LruCache::new(NonZeroUsize::new(512).unwrap()))),
            _phantom: PhantomData,
        }
    }

    /// Get the state key for the given key hash, if it is present in the keys table. If the key is
    /// not found in the keys table, it will return `None`. The result is also cached in an LRU
    /// cache to avoid localized, repeated lookups.
    fn get_key(&self, key_hash: KeyHash) -> Option<StateKey> {
        let mut keys_cache = self.keys_cache.lock().unwrap();

        if let Some(state_key) = keys_cache.get(&key_hash) {
            return Some(state_key.clone());
        }

        let state_key = self.keys_table.lock().unwrap().get(key_hash);
        if let Some(state_key) = state_key.clone() {
            keys_cache.put(key_hash, state_key.clone());
        }

        state_key
    }
}

impl<'a, B: StorageBackend, S: SerdeBackend> TreeReader for Adapter<'a, B, S> {
    /// Get the node for the given node key, if it is present in the tree.
    fn get_node_option(&self, node_key: &NodeKey) -> Result<Option<Node>> {
        let value = self.nodes_table.lock().unwrap().get(node_key);
        let value = match value {
            Some(value) => Some(value),
            None => {
                if node_key.nibble_path().is_empty() {
                    // If the nibble path is empty, it's a root node, and if it doesn't exist in the
                    // database, we should return a null node instead of None. This is needed for
                    // getting the root hash of the tree before any data has been inserted,
                    // otherwise the `jmt` crate panics.
                    Some(Node::Null)
                } else {
                    None
                }
            },
        };
        trace!(?node_key, "get_node_option");
        Ok(value)
    }

    /// Get the leftmost leaf node in the tree.
    /// This is not currently used, so it returns an error.
    fn get_rightmost_leaf(&self) -> Result<Option<(NodeKey, LeafNode)>> {
        unreachable!("Not currently used")
    }

    /// Get the state value for the given key hash, if it is present in the tree.
    fn get_value_option(
        &self,
        _max_version: Version,
        key_hash: KeyHash,
    ) -> Result<Option<OwnedValue>> {
        let state_key = self.get_key(key_hash);
        let value = if let Some(state_key) = state_key {
            self.ctx.get_raw_value(state_key.table, &state_key.key)
        } else {
            None
        };
        trace!(?key_hash, "get_value_option");
        Ok(value)
    }
}

impl<'a, B: StorageBackend, S: SerdeBackend> HasPreimage for Adapter<'a, B, S> {
    /// Gets the preimage of a key hash, if it is present in the tree.
    fn preimage(&self, key_hash: KeyHash) -> Result<Option<Vec<u8>>> {
        let state_key = self.get_key(key_hash);
        trace!(?key_hash, ?state_key, "preimage");
        Ok(state_key.map(|key| S::serialize(&key)))
    }
}
