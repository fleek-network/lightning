use std::collections::HashMap;
use std::marker::PhantomData;
use std::sync::{Arc, Mutex};

use anyhow::{ensure, Result};
use atomo::batch::{BoxedVec, Operation, VerticalBatch};
use atomo::{
    AtomoBuilder,
    InMemoryStorage,
    SerdeBackend,
    StorageBackend,
    StorageBackendConstructor,
    TableId,
    TableSelector,
};
use fxhash::FxHashMap;
use jmt::storage::{Node, NodeKey, TreeReader};
use jmt::{KeyHash, Version};
use tracing::{trace, trace_span};

use super::adapter::Adapter;
use super::hasher::SimpleHasherWrapper;
use super::proof::JmtStateProof;
use crate::providers::jmt::proof::ics23_proof_spec;
use crate::{MerklizeProvider, SimpleHasher, StateKey, StateRootHash, VerifyStateTreeError};

pub(crate) const NODES_TABLE_NAME: &str = "%state_tree_nodes";
pub(crate) const KEYS_TABLE_NAME: &str = "%state_tree_keys";

pub(crate) type SharedKeysTableRef<'a, B, S> =
    Arc<Mutex<atomo::TableRef<'a, KeyHash, StateKey, B, S>>>;
pub(crate) type SharedNodesTableRef<'a, B, S> =
    Arc<Mutex<atomo::TableRef<'a, NodeKey, Node, B, S>>>;

// The version of the JMT state tree.
// This needs to be greater than 0 because of the way we use the `jmt` crate without versioning. In
// `update_state_tree`, we insert the root node with version minus 1 to satisfy `jmt` crate
// expectations of retrieving the root of the previous version, which will panic if the version is
// 0. The `jmt` crate also has special handling of version 0, which we don't want to be in effect.
const TREE_VERSION: Version = 1;

#[derive(Debug, Clone)]
/// A merklize provider that uses a Jellyfish Merkle Tree (JMT) implementation ([`jmt`]) to manage
/// the database-backed state tree.
pub struct JmtMerklizeProvider<B: StorageBackend, S: SerdeBackend, H: SimpleHasher> {
    _phantom: PhantomData<(B, S, H)>,
}

impl<B: StorageBackend, S: SerdeBackend, H: SimpleHasher> JmtMerklizeProvider<B, S, H> {
    pub fn new() -> Self {
        Self {
            _phantom: PhantomData,
        }
    }
}

impl<B: StorageBackend, S: SerdeBackend, H: SimpleHasher> Default for JmtMerklizeProvider<B, S, H> {
    fn default() -> Self {
        Self::new()
    }
}

impl<B, S, H> MerklizeProvider for JmtMerklizeProvider<B, S, H>
where
    B: StorageBackend,
    S: SerdeBackend,
    H: SimpleHasher,
{
    type Storage = B;
    type Serde = S;
    type Hasher = H;
    type Proof = JmtStateProof;

    /// Augment the provided atomo builder with the necessary tables for the merklize provider.
    fn with_tables<C: StorageBackendConstructor>(
        builder: AtomoBuilder<C, S>,
    ) -> AtomoBuilder<C, S> {
        builder
            .with_table::<NodeKey, Node>(NODES_TABLE_NAME)
            .with_table::<KeyHash, StateKey>(KEYS_TABLE_NAME)
    }
    /// Get the state root hash of the state tree.
    /// Since we need to read the state, a table selector execution context is needed for
    /// consistency.
    fn get_state_root(ctx: &TableSelector<Self::Storage, Self::Serde>) -> Result<StateRootHash> {
        let span = trace_span!("get_state_root");
        let _enter = span.enter();

        let nodes_table = Arc::new(Mutex::new(ctx.get_table(NODES_TABLE_NAME)));
        let keys_table = Arc::new(Mutex::new(ctx.get_table(KEYS_TABLE_NAME)));

        let adapter = Adapter::new(ctx, nodes_table, keys_table);
        let tree = jmt::JellyfishMerkleTree::<_, SimpleHasherWrapper<H>>::new(&adapter);

        tree.get_root_hash(TREE_VERSION).map(|hash| hash.0.into())
    }

    /// Get an existence proof for the given key hash, if it is present in the state tree, or
    /// non-existence proof if it is not present.
    /// Since we need to read the state, a table selector execution context is needed for
    /// consistency.
    fn get_state_proof(
        ctx: &TableSelector<Self::Storage, Self::Serde>,
        table: &str,
        serialized_key: Vec<u8>,
    ) -> Result<JmtStateProof> {
        let span = trace_span!("get_state_proof");
        let _enter = span.enter();

        let nodes_table = Arc::new(Mutex::new(ctx.get_table(NODES_TABLE_NAME)));
        let keys_table = Arc::new(Mutex::new(ctx.get_table(KEYS_TABLE_NAME)));

        let adapter = Adapter::new(ctx, nodes_table, keys_table);
        let tree = jmt::JellyfishMerkleTree::<_, SimpleHasherWrapper<H>>::new(&adapter);

        let state_key = StateKey::new(table, serialized_key);
        let key_hash = state_key.hash::<S, H>();

        trace!(?key_hash, ?state_key, "get_state_proof");

        let (_value, proof) = tree.get_with_ics23_proof(
            S::serialize(&state_key),
            TREE_VERSION,
            ics23_proof_spec(H::ICS23_HASH_OP),
        )?;

        Ok(proof.into())
    }

    /// Apply the state tree changes based on the state changes in the atomo batch. This will update
    /// the state tree to reflect the changes in the atomo batch.
    /// Since we need to read the state, a table selector execution context is needed for
    /// consistency.
    fn update_state_tree<I>(
        ctx: &TableSelector<Self::Storage, Self::Serde>,
        batch: HashMap<String, I>,
    ) -> Result<()>
    where
        I: Iterator<Item = (BoxedVec, Operation)>,
    {
        let span = trace_span!("update_state_tree");
        let _enter = span.enter();

        let nodes_table = Arc::new(Mutex::new(ctx.get_table(NODES_TABLE_NAME)));
        let keys_table = Arc::new(Mutex::new(ctx.get_table(KEYS_TABLE_NAME)));

        let adapter = Adapter::new(ctx, nodes_table.clone(), keys_table.clone());
        let tree = jmt::JellyfishMerkleTree::<_, SimpleHasherWrapper<H>>::new(&adapter);

        // Build a jmt value set (batch) from the atomo batch.
        let mut value_set: Vec<(jmt::KeyHash, Option<jmt::OwnedValue>)> = Default::default();
        {
            let span = trace_span!("build_value_set");
            let _enter = span.enter();

            for (table, changes) in batch {
                if table == NODES_TABLE_NAME || table == KEYS_TABLE_NAME {
                    continue;
                }

                for (key, operation) in changes {
                    let state_key = StateKey::new(&table, key.to_vec());
                    let key_hash = jmt::KeyHash(state_key.hash::<S, H>().into());

                    match operation {
                        Operation::Remove => {
                            value_set.push((key_hash, None));

                            // Remove it from the keys table.
                            trace!(?key_hash, ?state_key, "removing key");
                            keys_table.lock().unwrap().remove(key_hash);
                        },
                        Operation::Insert(value) => {
                            let existing_value =
                                adapter.get_value_option(TREE_VERSION, key_hash)?;
                            if let Some(existing_value) = existing_value {
                                if existing_value == value.to_vec() {
                                    // If the key already exists with the same value, we
                                    // shouldn't insert it again. The storage backend deals with
                                    // this, but we should avoid inserting it into the tree
                                    // again. The `jmt` crate does not handle duplicate keys at
                                    // the moment, so we need do it here.
                                    break;
                                }
                            }

                            value_set.push((key_hash, Some(value.to_vec())));

                            // Insert it into the keys table.
                            trace!(?key_hash, ?state_key, "inserting key");
                            keys_table
                                .lock()
                                .unwrap()
                                .insert(key_hash, state_key.clone());
                        },
                    }
                }
            }
        }

        // Apply the jmt value set (batch) to the tree.
        let tree_batch = {
            let span = trace_span!("put_value_set");
            let _enter = span.enter();

            let (_new_root_hash, tree_batch) =
                tree.put_value_set(value_set.clone(), TREE_VERSION).unwrap();
            tree_batch
        };

        // Remove stale nodes.
        {
            let span = trace_span!("remove_stale_nodes");
            let _enter = span.enter();

            for stale_node in tree_batch.stale_node_index_batch {
                trace!(?stale_node, "removing stale node");

                nodes_table.lock().unwrap().remove(stale_node.node_key);
            }
        }

        // Insert new nodes.
        {
            let span = trace_span!("insert_new_nodes");
            let _enter = span.enter();

            for (node_key, node) in tree_batch.node_batch.nodes() {
                trace!(?node_key, ?node, "inserting new node");

                let mut nodes_table = nodes_table.lock().unwrap();

                if node_key.nibble_path().is_empty() {
                    // If the nibble path is empty, it's a root node and we should also insert it to
                    // the previous version, since `jmt` crate expects it, while our usage of `jmt`
                    // is with a single version.
                    let node_key =
                        NodeKey::new(node_key.version() - 1, node_key.nibble_path().clone());
                    nodes_table.insert(node_key, node);
                }

                nodes_table.insert(node_key, node);
            }
        }

        Ok(())
    }

    /// Clear the state tree by removing all nodes and keys from the atomo database.
    fn clear_state_tree(
        db: &mut atomo::Atomo<atomo::UpdatePerm, Self::Storage, Self::Serde>,
    ) -> Result<()> {
        let span = trace_span!("clear_state_tree");
        let _enter = span.enter();

        let tables = db.tables();
        let table_id_by_name = tables
            .iter()
            .enumerate()
            .map(|(tid, table)| (table.clone(), tid as TableId))
            .collect::<FxHashMap<_, _>>();

        let nodes_table_id = *table_id_by_name.get(NODES_TABLE_NAME).unwrap();
        let keys_table_id = *table_id_by_name.get(KEYS_TABLE_NAME).unwrap();

        let mut batch = VerticalBatch::new(tables.len());
        let storage = db.get_storage_backend_unsafe();

        // Remove nodes table entries.
        let nodes_table_batch = batch.get_mut(nodes_table_id as usize);
        for key in storage.keys(nodes_table_id) {
            nodes_table_batch.insert(key, Operation::Remove);
        }

        // Remove root table entries.
        let keys_table_batch = batch.get_mut(keys_table_id as usize);
        for key in storage.keys(keys_table_id) {
            keys_table_batch.insert(key, Operation::Remove);
        }

        // Commit the batch.
        storage.commit(batch);

        Ok(())
    }

    /// Verify that the state in the given atomo database instance, when used to build a new,
    /// temporary state tree from scratch, matches the stored state tree root hash.
    fn verify_state_tree(
        db: &mut atomo::Atomo<atomo::UpdatePerm, Self::Storage, Self::Serde>,
    ) -> Result<()> {
        let span = trace_span!("verify_state_tree");
        let _enter = span.enter();

        // Build batch of all state data.
        let tables = db.tables();
        let mut batch = HashMap::new();
        for (i, table) in tables.clone().into_iter().enumerate() {
            let tid = i as u8;

            let mut changes = Vec::new();
            for (key, value) in db.get_storage_backend_unsafe().get_all(tid) {
                changes.push((key, Operation::Insert(value)));
            }
            batch.insert(table, changes.into_iter());
        }

        // Build a new, temporary state tree from the batch.
        let builder = AtomoBuilder::<_, S>::new(InMemoryStorage::default());
        type TempDbProvider<S, H> = JmtMerklizeProvider<InMemoryStorage, S, H>;
        let mut tmp_db = TempDbProvider::<S, H>::with_tables(builder).build()?;
        tmp_db.run(|ctx| TempDbProvider::<S, H>::update_state_tree(ctx, batch))?;

        // Get and return the state root hash from the temporary state tree.
        let tmp_state_root = tmp_db
            .query()
            .run(|ctx| TempDbProvider::<S, H>::get_state_root(ctx))?;

        // Check that the state root hash matches the stored state root hash.
        let stored_state_root = db.query().run(|ctx| Self::get_state_root(ctx))?;
        ensure!(
            tmp_state_root == stored_state_root,
            VerifyStateTreeError::StateRootMismatch(stored_state_root, tmp_state_root)
        );

        Ok(())
    }
}
