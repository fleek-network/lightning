use std::marker::PhantomData;
use std::sync::{Arc, Mutex};

use anyhow::{anyhow, Result};
use atomo::batch::Operation;
use atomo::{SerdeBackend, StorageBackend, TableId, TableSelector};
use fxhash::FxHashMap;
use tracing::{trace, trace_span};
use trie_db::proof::generate_proof;
use trie_db::{DBValue, TrieDBMutBuilder, TrieHash, TrieMut};

use super::adapter::Adapter;
use super::hasher::SimpleHasherWrapper;
use super::layout::TrieLayoutWrapper;
use super::provider::{NODES_TABLE_NAME, ROOT_TABLE_NAME};
use super::MptStateProof;
use crate::{MerklizeContext, SimpleHasher, StateKey, StateRootHash};

type SharedTableRef<'a, K, V, B, S> = Arc<Mutex<atomo::TableRef<'a, K, V, B, S>>>;

/// A merklize context that can be used to read and update tables of data, wrapping an
/// `[atomo::TableSelector]` instance to provide similar functionality, but with additional
/// merklize state tree features.
pub struct MptMerklizeContext<'a, B: StorageBackend, S: SerdeBackend, H>
where
    H: SimpleHasher + Send + Sync,
{
    ctx: &'a TableSelector<B, S>,
    table_name_by_id: FxHashMap<TableId, String>,
    nodes_table:
        SharedTableRef<'a, <SimpleHasherWrapper<H> as hash_db::Hasher>::Out, DBValue, B, S>,
    root_table: SharedTableRef<'a, u8, StateRootHash, B, S>,
    _phantom: PhantomData<H>,
}

impl<'a, B: StorageBackend, S: SerdeBackend, H> MptMerklizeContext<'a, B, S, H>
where
    H: SimpleHasher + Send + Sync,
{
    /// Create a new merklize context for the given table selector, initializing state tree tables
    /// and other necessary data for the context functionality.
    pub fn new(ctx: &'a TableSelector<B, S>) -> Self {
        let tables = ctx.tables();

        let nodes_table = ctx.get_table(NODES_TABLE_NAME);
        let root_table = ctx.get_table(ROOT_TABLE_NAME);

        let mut table_id_by_name = FxHashMap::default();
        for (i, table) in tables.iter().enumerate() {
            let table_id: TableId = i.try_into().unwrap();
            let table_name = table.name.to_string();
            table_id_by_name.insert(table_name, table_id);
        }

        let table_name_by_id = table_id_by_name
            .clone()
            .into_iter()
            .map(|(k, v)| (v, k))
            .collect::<FxHashMap<TableId, String>>();

        Self {
            ctx,
            table_name_by_id,
            nodes_table: Arc::new(Mutex::new(nodes_table)),
            root_table: Arc::new(Mutex::new(root_table)),
            _phantom: PhantomData,
        }
    }

    /// Read the state root hash from the root table.
    fn read_root(&self) -> Option<StateRootHash> {
        // We only store the latest root hash in the root table, and so we just use the key 0u8.
        self.root_table.lock().unwrap().get(0)
    }

    /// Write the given state root to the root table.
    fn write_root(&self, root: StateRootHash) {
        // We only store the latest root hash in the root table, and so we just use the key 0u8.
        self.root_table.lock().unwrap().insert(0, root);
    }
}

impl<'a, B, S, H> MerklizeContext<'a, B, S, H, MptStateProof> for MptMerklizeContext<'a, B, S, H>
where
    B: StorageBackend + Send + Sync,
    S: SerdeBackend + Send + Sync,
    H: SimpleHasher + Send + Sync,
{
    /// Get the state root hash of the state tree.
    fn get_state_root(&self) -> Result<StateRootHash> {
        let span = trace_span!("get_state_root");
        let _enter = span.enter();

        let root = self.read_root();

        if let Some(root) = root {
            Ok(root)
        } else {
            let mut root: <SimpleHasherWrapper<H> as hash_db::Hasher>::Out =
                StateRootHash::default().into();
            let mut adapter = Adapter::<B, S, H>::new(self.nodes_table.clone());
            let mut tree =
                TrieDBMutBuilder::<TrieLayoutWrapper<H>>::new(&mut adapter, &mut root).build();

            // Note that tree.root() calls tree.commit() before returning the root hash.
            let root = *tree.root();

            // Save the root hash to the root table.
            self.write_root(root.into());

            Ok(root.into())
        }
    }

    /// Get an existence proof for the given key hash, if it is present in the state tree, or
    /// non-existence proof if it is not present. The proof will include the value if it exists.
    fn get_state_proof(&self, table: &str, serialized_key: Vec<u8>) -> Result<MptStateProof> {
        let span = trace_span!("get_state_proof");
        let _enter = span.enter();

        let state_root: <SimpleHasherWrapper<H> as hash_db::Hasher>::Out =
            self.get_state_root()?.into();
        let adapter = Adapter::<B, S, H>::new(self.nodes_table.clone());

        let state_key = StateKey::new(table, serialized_key);
        let key_hash = state_key.hash::<S, H>();
        trace!(?key_hash, ?state_key, "get_state_proof");
        let key_hash: TrieHash<TrieLayoutWrapper<H>> = key_hash.into();

        let proof =
            generate_proof::<_, TrieLayoutWrapper<H>, _, _>(&adapter, &state_root, &vec![key_hash])
                .unwrap();
        let proof = MptStateProof::new(proof);

        Ok(proof)
    }

    /// Apply the state tree changes based on the state changes in the atomo batch. This will update
    /// the state tree to reflect the changes in the atomo batch. It reads data from the state tree,
    /// so an execution context is needed to ensure consistency.
    fn apply_state_tree_changes(&mut self) -> Result<()> {
        let span = trace_span!("apply_state_tree_changes");
        let _enter = span.enter();

        let mut state_root: <SimpleHasherWrapper<H> as hash_db::Hasher>::Out =
            self.get_state_root()?.into();
        let mut adapter = Adapter::<B, S, H>::new(self.nodes_table.clone());
        let mut tree =
            TrieDBMutBuilder::<TrieLayoutWrapper<H>>::from_existing(&mut adapter, &mut state_root)
                .build();

        let batch = self.ctx.batch();
        for (table_id, changes) in batch.into_raw().iter().enumerate() {
            let table_id: TableId = table_id.try_into()?;
            let table_name = self
                .table_name_by_id
                .get(&table_id)
                .ok_or(anyhow!("Table with index {} not found", table_id))?
                .as_str();
            for (key, operation) in changes.iter() {
                let state_key = StateKey::new(table_name, key.to_vec());
                let key_hash = state_key.hash::<S, H>();

                match operation {
                    Operation::Remove => {
                        tree.remove(key_hash.as_ref())?;
                    },
                    Operation::Insert(value) => {
                        tree.insert(key_hash.as_ref(), value)?;
                    },
                }
            }
        }

        {
            let span = trace_span!("triedb.commit");
            let _enter = span.enter();

            // Note that tree.root() calls tree.commit() before returning the root hash, so we don't
            // need to explicitly `tree.commit()` here, but otherwise would.
            let root = *tree.root();

            // Save the root hash to the root table.
            self.write_root(root.into());
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use atomo::{
        Atomo,
        AtomoBuilder,
        DefaultSerdeBackend,
        InMemoryStorage,
        StorageBackendConstructor,
        UpdatePerm,
    };

    use super::*;
    use crate::hashers::sha2::Sha256Hasher;
    use crate::providers::mpt::MptMerklizeProvider;
    use crate::StateProof as _;

    fn build_atomo<C: StorageBackendConstructor, S: SerdeBackend, H: SimpleHasher>(
        builder: C,
    ) -> Atomo<UpdatePerm, C::Storage, S> {
        AtomoBuilder::<_, S>::new(builder)
            .with_table::<String, String>("data")
            .with_table::<<SimpleHasherWrapper<H> as hash_db::Hasher>::Out, DBValue>(
                NODES_TABLE_NAME,
            )
            .with_table::<u8, StateRootHash>(ROOT_TABLE_NAME)
            .build()
            .unwrap()
    }

    #[test]
    fn test_apply_state_tree_changes_with_updates() {
        type S = DefaultSerdeBackend;
        type H = Sha256Hasher;

        let mut db = build_atomo::<_, S, H>(InMemoryStorage::default());

        // Check storage.
        {
            let storage = db.get_storage_backend_unsafe();
            assert_eq!(storage.keys(1).len(), 0); // nodes
            assert_eq!(storage.keys(2).len(), 0); // root
        }

        // Insert a value.
        db.run(|ctx| {
            let mut table = ctx.get_table::<String, String>("data");

            table.insert("key1".to_string(), "value1".to_string());

            MptMerklizeContext::<_, _, H>::new(ctx)
                .apply_state_tree_changes()
                .unwrap();
        });

        // Check storage.
        {
            let storage = db.get_storage_backend_unsafe();
            assert_eq!(storage.keys(1).len(), 3); // nodes
            assert_eq!(storage.keys(2).len(), 1); // root
        }

        // Insert another value.
        db.run(|ctx| {
            let mut table = ctx.get_table::<String, String>("data");

            table.insert("key2".to_string(), "value2".to_string());

            MptMerklizeContext::<_, _, H>::new(ctx)
                .apply_state_tree_changes()
                .unwrap();
        });

        // Check storage.
        {
            let storage = db.get_storage_backend_unsafe();
            assert_eq!(storage.keys(1).len(), 4); // nodes
            assert_eq!(storage.keys(2).len(), 1); // root
        }

        // Remove a value.
        db.run(|ctx| {
            let mut table = ctx.get_table::<String, String>("data");

            table.remove("key2".to_string());

            MptMerklizeContext::<_, _, H>::new(ctx)
                .apply_state_tree_changes()
                .unwrap();
        });

        // Check storage.
        {
            let storage = db.get_storage_backend_unsafe();
            assert_eq!(storage.keys(1).len(), 3); // nodes
            assert_eq!(storage.keys(2).len(), 1); // root
        }

        // Insert removed key with different value.
        db.run(|ctx| {
            let mut table = ctx.get_table::<String, String>("data");

            table.insert("key2".to_string(), "other-value2".to_string());

            MptMerklizeContext::<_, _, H>::new(ctx)
                .apply_state_tree_changes()
                .unwrap();
        });

        // Check storage.
        {
            let storage = db.get_storage_backend_unsafe();
            assert_eq!(storage.keys(1).len(), 4); // nodes
            assert_eq!(storage.keys(2).len(), 1); // root
        }

        // Insert existing key with same value.
        db.run(|ctx| {
            let mut table = ctx.get_table::<String, String>("data");

            table.insert("key1".to_string(), "value1".to_string());

            MptMerklizeContext::<_, _, H>::new(ctx)
                .apply_state_tree_changes()
                .unwrap();
        });

        // Check storage.
        {
            let storage = db.get_storage_backend_unsafe();
            assert_eq!(storage.keys(1).len(), 4); // nodes
            assert_eq!(storage.keys(2).len(), 1); // root
        }
    }

    #[test]
    fn test_apply_state_tree_changes_with_no_changes() {
        type S = DefaultSerdeBackend;
        type H = Sha256Hasher;

        let mut db = build_atomo::<_, S, H>(InMemoryStorage::default());

        // Check storage.
        {
            let storage = db.get_storage_backend_unsafe();
            assert_eq!(storage.keys(1).len(), 0); // nodes
            assert_eq!(storage.keys(2).len(), 0); // root
        }

        // Open run context and apply state tree changes, but don't make any state changes before.
        db.run(|ctx| {
            MptMerklizeContext::<_, _, H>::new(ctx)
                .apply_state_tree_changes()
                .unwrap();
        });

        // Check storage.
        {
            let storage = db.get_storage_backend_unsafe();
            assert_eq!(storage.keys(1).len(), 1); // nodes
            assert_eq!(storage.keys(2).len(), 1); // root
        }

        // Insert another value.
        db.run(|ctx| {
            let mut table = ctx.get_table::<String, String>("data");

            table.insert("key2".to_string(), "value2".to_string());

            MptMerklizeContext::<_, _, H>::new(ctx)
                .apply_state_tree_changes()
                .unwrap();
        });

        // Check storage.
        {
            let storage = db.get_storage_backend_unsafe();
            assert_eq!(storage.keys(1).len(), 3); // nodes
            assert_eq!(storage.keys(2).len(), 1); // root
        }
    }

    #[test]
    fn test_get_state_root_with_empty_state() {
        type S = DefaultSerdeBackend;
        type H = Sha256Hasher;

        let db = build_atomo::<_, S, H>(InMemoryStorage::default());
        let query = db.query();

        let state_root = query.run(|ctx| {
            MptMerklizeContext::<_, _, H>::new(ctx)
                .get_state_root()
                .unwrap()
        });
        assert_eq!(
            state_root,
            "6e340b9cffb37a989ca544e6bb780a2c78901d3fb33738768511a30617afa01d"
        );
    }

    #[test]
    fn test_get_state_root_with_updates() {
        type S = DefaultSerdeBackend;
        type H = Sha256Hasher;

        let mut db = build_atomo::<_, S, H>(InMemoryStorage::default());
        let query = db.query();

        // Check the state root hash.
        let empty_state_root = "6e340b9cffb37a989ca544e6bb780a2c78901d3fb33738768511a30617afa01d";
        let initial_state_root = query.run(|ctx| {
            MptMerklizeContext::<_, _, H>::new(ctx)
                .get_state_root()
                .unwrap()
        });
        assert_ne!(initial_state_root, StateRootHash::default());
        assert_eq!(initial_state_root, empty_state_root);

        // Insert a value.
        db.run(|ctx| {
            let mut table = ctx.get_table::<String, String>("data");

            table.insert("key1".to_string(), "value1".to_string());

            MptMerklizeContext::<_, _, H>::new(ctx)
                .apply_state_tree_changes()
                .unwrap();
        });

        // Check the state root hash.
        let new_state_root = query.run(|ctx| {
            MptMerklizeContext::<_, _, H>::new(ctx)
                .get_state_root()
                .unwrap()
        });
        assert_ne!(new_state_root, StateRootHash::default());
        assert_ne!(initial_state_root, new_state_root);
        let old_state_root = new_state_root;

        // Insert another value.
        db.run(|ctx| {
            let mut table = ctx.get_table::<String, String>("data");

            table.insert("key2".to_string(), "value2".to_string());

            MptMerklizeContext::<_, _, H>::new(ctx)
                .apply_state_tree_changes()
                .unwrap();
        });

        // Check the state root hash.
        let new_state_root = query.run(|ctx| {
            MptMerklizeContext::<_, _, H>::new(ctx)
                .get_state_root()
                .unwrap()
        });
        assert_ne!(new_state_root, StateRootHash::default());
        assert_ne!(old_state_root, new_state_root);
        let old_state_root = new_state_root;

        // Remove a key.
        db.run(|ctx| {
            let mut table = ctx.get_table::<String, String>("data");

            table.remove("key2".to_string());

            MptMerklizeContext::<_, _, H>::new(ctx)
                .apply_state_tree_changes()
                .unwrap();
        });

        // Check the state root hash.
        let new_state_root = query.run(|ctx| {
            MptMerklizeContext::<_, _, H>::new(ctx)
                .get_state_root()
                .unwrap()
        });
        assert_ne!(new_state_root, StateRootHash::default());
        assert_ne!(old_state_root, new_state_root);
        let old_state_root = new_state_root;

        // Remove same key.
        db.run(|ctx| {
            let mut table = ctx.get_table::<String, String>("data");

            table.remove("key2".to_string());

            MptMerklizeContext::<_, _, H>::new(ctx)
                .apply_state_tree_changes()
                .unwrap();
        });

        // Check the state root hash.
        let new_state_root = query.run(|ctx| {
            MptMerklizeContext::<_, _, H>::new(ctx)
                .get_state_root()
                .unwrap()
        });
        assert_eq!(old_state_root, new_state_root);
        let old_state_root = new_state_root;

        // Insert removed key with different value.
        db.run(|ctx| {
            let mut table = ctx.get_table::<String, String>("data");

            table.insert("key2".to_string(), "other-value2".to_string());

            MptMerklizeContext::<_, _, H>::new(ctx)
                .apply_state_tree_changes()
                .unwrap();
        });

        // Check the state root hash.
        let new_state_root = query.run(|ctx| {
            MptMerklizeContext::<_, _, H>::new(ctx)
                .get_state_root()
                .unwrap()
        });
        assert_ne!(new_state_root, StateRootHash::default());
        assert_ne!(old_state_root, new_state_root);
        let old_state_root = new_state_root;

        // Insert existing key with same value.
        db.run(|ctx| {
            let mut table = ctx.get_table::<String, String>("data");

            table.insert("key1".to_string(), "value1".to_string());

            MptMerklizeContext::<_, _, H>::new(ctx)
                .apply_state_tree_changes()
                .unwrap();
        });

        // Check the state root hash.
        let new_state_root = query.run(|ctx| {
            MptMerklizeContext::<_, _, H>::new(ctx)
                .get_state_root()
                .unwrap()
        });
        assert_eq!(old_state_root, new_state_root);
        let old_state_root = new_state_root;

        // Insert existing key with different value.
        db.run(|ctx| {
            let mut table = ctx.get_table::<String, String>("data");

            table.insert("key1".to_string(), "other-value1".to_string());

            MptMerklizeContext::<_, _, H>::new(ctx)
                .apply_state_tree_changes()
                .unwrap();
        });

        // Check the state root hash.
        let new_state_root = query.run(|ctx| {
            MptMerklizeContext::<_, _, H>::new(ctx)
                .get_state_root()
                .unwrap()
        });
        assert_ne!(old_state_root, new_state_root);
        let old_state_root = new_state_root;

        // Remove non-existent key.
        db.run(|ctx| {
            let mut table = ctx.get_table::<String, String>("data");

            table.remove("unknown".to_string());

            MptMerklizeContext::<_, _, H>::new(ctx)
                .apply_state_tree_changes()
                .unwrap();
        });

        // Check the state root hash.
        let new_state_root = query.run(|ctx| {
            MptMerklizeContext::<_, _, H>::new(ctx)
                .get_state_root()
                .unwrap()
        });
        assert_eq!(old_state_root, new_state_root);
    }

    #[test]
    fn test_get_state_proof_of_membership() {
        type S = DefaultSerdeBackend;
        type H = Sha256Hasher;
        type M = MptMerklizeProvider<InMemoryStorage, S, H>;

        let _ = tracing_subscriber::fmt::try_init();

        let mut db = build_atomo::<_, S, H>(InMemoryStorage::default());
        let query = db.query();

        // Get a proof of non-membership with empty state, should fail.
        let state_root = query.run(|ctx| {
            MptMerklizeContext::<_, _, H>::new(ctx)
                .get_state_root()
                .unwrap()
        });
        let proof = query
            .run(|ctx| {
                MptMerklizeContext::<_, _, H>::new(ctx)
                    .get_state_proof("data", S::serialize(&"key1".to_string()))
            })
            .unwrap();
        proof
            .verify_non_membership::<String, M>("data", "key1".to_string(), state_root)
            .unwrap();

        // Insert a value.
        db.run(|ctx| {
            let mut table = ctx.get_table::<String, String>("data");

            table.insert("key1".to_string(), "value1".to_string());
            table.insert("key2".to_string(), "value2".to_string());
            table.insert("key3".to_string(), "value3".to_string());

            MptMerklizeContext::<_, _, H>::new(ctx)
                .apply_state_tree_changes()
                .unwrap();
        });

        // Get state root for proof verification.
        let state_root = query.run(|ctx| {
            MptMerklizeContext::<_, _, H>::new(ctx)
                .get_state_root()
                .unwrap()
        });

        // Get and verify proof of membership.
        let proof = query.run(|ctx| {
            MptMerklizeContext::<_, _, H>::new(ctx)
                .get_state_proof("data", S::serialize(&"key1".to_string()))
                .unwrap()
        });
        proof
            .verify_membership::<String, String, M>(
                "data",
                "key1".to_string(),
                "value1".to_string(),
                state_root,
            )
            .unwrap();

        // Get and verify proof of non-membership of unknown key.
        let proof = query.run(|ctx| {
            MptMerklizeContext::<_, _, H>::new(ctx)
                .get_state_proof("data", S::serialize(&"unknown".to_string()))
                .unwrap()
        });
        proof
            .verify_non_membership::<String, M>("data", "unknown".to_string(), state_root)
            .unwrap();

        // Remove a value.
        db.run(|ctx| {
            let mut table = ctx.get_table::<String, String>("data");

            table.remove("key2".to_string());

            MptMerklizeContext::<_, _, H>::new(ctx)
                .apply_state_tree_changes()
                .unwrap();
        });

        // Get state root for proof verification.
        let state_root = query.run(|ctx| {
            MptMerklizeContext::<_, _, H>::new(ctx)
                .get_state_root()
                .unwrap()
        });

        // Get and verify proof of non-membership of removed key.
        let proof = query.run(|ctx| {
            MptMerklizeContext::<_, _, H>::new(ctx)
                .get_state_proof("data", S::serialize(&"key2".to_string()))
                .unwrap()
        });
        proof
            .verify_non_membership::<String, M>("data", "key2".to_string(), state_root)
            .unwrap();
    }
}
