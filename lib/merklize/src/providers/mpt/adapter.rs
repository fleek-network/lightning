use std::marker::PhantomData;

use atomo::{SerdeBackend, StorageBackend};
use hash_db::{AsHashDB, HashDB, HashDBRef, Prefix};
use tracing::{trace, trace_span};
use trie_db::DBValue;

use super::hasher::SimpleHasherWrapper;
use super::provider::SharedNodesTableRef;
use crate::SimpleHasher;

/// A HashDB implementation that uses a given atomo state tree nodes table to read and write nodes
/// as required by the HashDB trait. This acts as an adapter between a merklize provider
/// functionality and the implementation using `TrieDB`.
pub(crate) struct Adapter<'a, B, S, H>
where
    B: StorageBackend + Send + Sync,
    S: SerdeBackend + Send + Sync,
    H: SimpleHasher + Send + Sync,
{
    nodes_table: SharedNodesTableRef<'a, B, S, H>,
    hashed_null_node: <SimpleHasherWrapper<H> as hash_db::Hasher>::Out,
    null_node_data: DBValue,
    _phantom: PhantomData<()>,
}

impl<'a, B, S, H> Adapter<'a, B, S, H>
where
    B: StorageBackend + Send + Sync,
    S: SerdeBackend + Send + Sync,
    H: SimpleHasher + Send + Sync,
{
    pub fn new(nodes_table: SharedNodesTableRef<'a, B, S, H>) -> Self {
        let null = &[0u8][..];
        Self {
            nodes_table,
            hashed_null_node: H::hash(null),
            null_node_data: null.into(),
            _phantom: PhantomData,
        }
    }
}

impl<'a, B, S, H> HashDB<SimpleHasherWrapper<H>, DBValue> for Adapter<'a, B, S, H>
where
    B: StorageBackend + Send + Sync,
    S: SerdeBackend + Send + Sync,
    H: SimpleHasher + Send + Sync,
{
    fn get(
        &self,
        key: &<SimpleHasherWrapper<H> as hash_db::Hasher>::Out,
        prefix: Prefix,
    ) -> Option<DBValue> {
        let span = trace_span!("get");
        let _enter = span.enter();
        trace!(key = hex::encode(key), ?prefix, "get");

        if key == &self.hashed_null_node {
            return Some(self.null_node_data.clone());
        }

        self.nodes_table.lock().unwrap().get(key)
    }

    fn contains(
        &self,
        key: &<SimpleHasherWrapper<H> as hash_db::Hasher>::Out,
        prefix: Prefix,
    ) -> bool {
        let span = trace_span!("contains");
        let _enter = span.enter();

        if key == &self.hashed_null_node {
            trace!(key = hex::encode(key), ?prefix, "contains with null node");
            return true;
        }

        trace!(key = hex::encode(key), ?prefix, "contains");
        self.nodes_table.lock().unwrap().contains_key(key)
    }

    fn emplace(
        &mut self,
        key: <SimpleHasherWrapper<H> as hash_db::Hasher>::Out,
        prefix: Prefix,
        value: DBValue,
    ) {
        let span = trace_span!("emplace");
        let _enter = span.enter();

        if value == self.null_node_data {
            trace!(key = hex::encode(key), ?prefix, "emplace with null node");
            return;
        }

        trace!(key = hex::encode(key), ?prefix, ?value, "emplace");
        self.nodes_table.lock().unwrap().insert(key, value);
    }

    fn insert(
        &mut self,
        prefix: Prefix,
        value: &[u8],
    ) -> <SimpleHasherWrapper<H> as hash_db::Hasher>::Out {
        let span = trace_span!("insert");
        let _enter = span.enter();

        if value == self.null_node_data {
            trace!(?prefix, ?value, "insert with null node");
            return self.hashed_null_node;
        }

        let key = H::hash(value);

        trace!(key = hex::encode(key), ?prefix, ?value, "insert");
        self.nodes_table.lock().unwrap().insert(key, value.to_vec());

        key
    }

    fn remove(&mut self, key: &<SimpleHasherWrapper<H> as hash_db::Hasher>::Out, prefix: Prefix) {
        let span = trace_span!("remove");
        let _enter = span.enter();

        trace!(key = hex::encode(key), ?prefix, "remove");
        self.nodes_table.lock().unwrap().remove(key);
    }
}

impl<'a, B, S, H> HashDBRef<SimpleHasherWrapper<H>, DBValue> for Adapter<'a, B, S, H>
where
    B: StorageBackend + Send + Sync,
    S: SerdeBackend + Send + Sync,
    H: SimpleHasher + Send + Sync,
{
    fn get(
        &self,
        key: &<SimpleHasherWrapper<H> as hash_db::Hasher>::Out,
        prefix: Prefix,
    ) -> Option<DBValue> {
        HashDB::get(self, key, prefix)
    }
    fn contains(
        &self,
        key: &<SimpleHasherWrapper<H> as hash_db::Hasher>::Out,
        prefix: Prefix,
    ) -> bool {
        HashDB::contains(self, key, prefix)
    }
}

impl<'a, B, S, H> AsHashDB<SimpleHasherWrapper<H>, DBValue> for Adapter<'a, B, S, H>
where
    B: StorageBackend + Send + Sync,
    S: SerdeBackend + Send + Sync,
    H: SimpleHasher + Send + Sync,
{
    fn as_hash_db(&self) -> &dyn HashDB<SimpleHasherWrapper<H>, DBValue> {
        self
    }
    fn as_hash_db_mut(&mut self) -> &mut dyn HashDB<SimpleHasherWrapper<H>, DBValue> {
        self
    }
}
