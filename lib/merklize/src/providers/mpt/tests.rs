use atomo::{AtomoBuilder, DefaultSerdeBackend, InMemoryStorage, SerdeBackend, StorageBackend};

use crate::hashers::sha2::Sha256Hasher;
use crate::proof::StateProof;
use crate::provider::MerklizeProvider;
use crate::providers::mpt::MptMerklizeProvider;
use crate::StateRootHash;

#[test]
fn test_mpt_update_state_tree_with_updates() {
    type S = DefaultSerdeBackend;
    type H = Sha256Hasher;
    type M = MptMerklizeProvider<InMemoryStorage, S, H>;

    let builder = AtomoBuilder::new(InMemoryStorage::default());
    let mut db = M::with_tables(builder.with_table::<String, String>("data"))
        .build()
        .unwrap();

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

        M::update_state_tree_from_context(ctx).unwrap();
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

        M::update_state_tree_from_context(ctx).unwrap();
    });

    // Check storage.
    {
        let storage = db.get_storage_backend_unsafe();
        assert_eq!(storage.keys(1).len(), 3); // nodes
        assert_eq!(storage.keys(2).len(), 1); // root
    }

    // Remove a value.
    db.run(|ctx| {
        let mut table = ctx.get_table::<String, String>("data");

        table.remove("key2".to_string());

        M::update_state_tree_from_context(ctx).unwrap();
    });

    // Check storage.
    {
        let storage = db.get_storage_backend_unsafe();
        assert_eq!(storage.keys(1).len(), 1); // nodes
        assert_eq!(storage.keys(2).len(), 1); // root
    }

    // Insert removed key with different value.
    db.run(|ctx| {
        let mut table = ctx.get_table::<String, String>("data");

        table.insert("key2".to_string(), "other-value2".to_string());

        M::update_state_tree_from_context(ctx).unwrap();
    });

    // Check storage.
    {
        let storage = db.get_storage_backend_unsafe();
        assert_eq!(storage.keys(1).len(), 3); // nodes
        assert_eq!(storage.keys(2).len(), 1); // root
    }

    // Insert existing key with same value.
    db.run(|ctx| {
        let mut table = ctx.get_table::<String, String>("data");

        table.insert("key1".to_string(), "value1".to_string());

        M::update_state_tree_from_context(ctx).unwrap();
    });

    // Check storage.
    {
        let storage = db.get_storage_backend_unsafe();
        assert_eq!(storage.keys(1).len(), 3); // nodes
        assert_eq!(storage.keys(2).len(), 1); // root
    }
}

#[test]
fn test_mpt_update_state_tree_with_no_changes() {
    type S = DefaultSerdeBackend;
    type H = Sha256Hasher;
    type M = MptMerklizeProvider<InMemoryStorage, S, H>;

    let builder = AtomoBuilder::new(InMemoryStorage::default());
    let mut db = M::with_tables(builder.with_table::<String, String>("data"))
        .build()
        .unwrap();

    // Check storage.
    {
        let storage = db.get_storage_backend_unsafe();
        assert_eq!(storage.keys(1).len(), 0); // nodes
        assert_eq!(storage.keys(2).len(), 0); // root
    }

    // Open run context and apply state tree changes, but don't make any state changes before.
    db.run(|ctx| {
        // Do nothing.

        M::update_state_tree_from_context(ctx).unwrap();
    });

    // Check storage.
    {
        let storage = db.get_storage_backend_unsafe();
        assert_eq!(storage.keys(1).len(), 0); // nodes
        assert_eq!(storage.keys(2).len(), 1); // root
    }

    // Insert another value.
    db.run(|ctx| {
        let mut table = ctx.get_table::<String, String>("data");

        table.insert("key2".to_string(), "value2".to_string());

        M::update_state_tree_from_context(ctx).unwrap();
    });

    // Check storage.
    {
        let storage = db.get_storage_backend_unsafe();
        assert_eq!(storage.keys(1).len(), 1); // nodes
        assert_eq!(storage.keys(2).len(), 1); // root
    }
}

#[test]
fn test_mpt_get_state_root_with_empty_state() {
    type S = DefaultSerdeBackend;
    type H = Sha256Hasher;
    type M = MptMerklizeProvider<InMemoryStorage, S, H>;

    let builder = AtomoBuilder::new(InMemoryStorage::default());
    let db = M::with_tables(builder.with_table::<String, String>("data"))
        .build()
        .unwrap();
    let query = db.query();

    let state_root = query.run(|ctx| M::get_state_root(ctx).unwrap());
    assert_eq!(
        state_root,
        "6e340b9cffb37a989ca544e6bb780a2c78901d3fb33738768511a30617afa01d"
    );
}

#[test]
fn test_mpt_get_state_root_with_updates() {
    type S = DefaultSerdeBackend;
    type H = Sha256Hasher;
    type M = MptMerklizeProvider<InMemoryStorage, S, H>;

    let builder = AtomoBuilder::new(InMemoryStorage::default());
    let mut db = M::with_tables(builder.with_table::<String, String>("data"))
        .build()
        .unwrap();
    let query = db.query();

    // Check the state root hash.
    let empty_state_root = "6e340b9cffb37a989ca544e6bb780a2c78901d3fb33738768511a30617afa01d";
    let initial_state_root = query.run(|ctx| M::get_state_root(ctx).unwrap());
    assert_ne!(initial_state_root, StateRootHash::default());
    assert_eq!(initial_state_root, empty_state_root);

    // Insert a value.
    db.run(|ctx| {
        let mut table = ctx.get_table::<String, String>("data");

        table.insert("key1".to_string(), "value1".to_string());

        M::update_state_tree_from_context(ctx).unwrap();
    });

    // Check the state root hash.
    let new_state_root = query.run(|ctx| M::get_state_root(ctx).unwrap());
    assert_ne!(new_state_root, StateRootHash::default());
    assert_ne!(initial_state_root, new_state_root);
    let old_state_root = new_state_root;

    // Verify the state tree by rebuilding it and comparing the root hashes.
    M::verify_state_tree(&mut db).unwrap();

    // Insert another value.
    db.run(|ctx| {
        let mut table = ctx.get_table::<String, String>("data");

        table.insert("key2".to_string(), "value2".to_string());

        M::update_state_tree_from_context(ctx).unwrap();

        // Check that we can get the current root hash before the commit happens.
        let state_root = M::get_state_root(ctx).unwrap();
        assert_ne!(state_root, StateRootHash::default());
        assert_ne!(old_state_root, state_root);
    });

    // Check the state root hash.
    let new_state_root = query.run(|ctx| M::get_state_root(ctx).unwrap());
    assert_ne!(new_state_root, StateRootHash::default());
    assert_ne!(old_state_root, new_state_root);
    let old_state_root = new_state_root;

    // Remove a key.
    db.run(|ctx| {
        let mut table = ctx.get_table::<String, String>("data");

        table.remove("key2".to_string());

        M::update_state_tree_from_context(ctx).unwrap();
    });

    // Check the state root hash.
    let new_state_root = query.run(|ctx| M::get_state_root(ctx).unwrap());
    assert_ne!(new_state_root, StateRootHash::default());
    assert_ne!(old_state_root, new_state_root);
    let old_state_root = new_state_root;

    // Remove same key.
    db.run(|ctx| {
        let mut table = ctx.get_table::<String, String>("data");

        table.remove("key2".to_string());

        M::update_state_tree_from_context(ctx).unwrap();
    });

    // Check the state root hash.
    let new_state_root = query.run(|ctx| M::get_state_root(ctx).unwrap());
    assert_eq!(old_state_root, new_state_root);
    let old_state_root = new_state_root;

    // Insert removed key with different value.
    db.run(|ctx| {
        let mut table = ctx.get_table::<String, String>("data");

        table.insert("key2".to_string(), "other-value2".to_string());

        M::update_state_tree_from_context(ctx).unwrap();
    });

    // Check the state root hash.
    let new_state_root = query.run(|ctx| M::get_state_root(ctx).unwrap());
    assert_ne!(new_state_root, StateRootHash::default());
    assert_ne!(old_state_root, new_state_root);
    let old_state_root = new_state_root;

    // Verify the state tree by rebuilding it and comparing the root hashes.
    M::verify_state_tree(&mut db).unwrap();

    // Insert existing key with same value.
    db.run(|ctx| {
        let mut table = ctx.get_table::<String, String>("data");

        table.insert("key1".to_string(), "value1".to_string());

        M::update_state_tree_from_context(ctx).unwrap();
    });

    // Check the state root hash.
    let new_state_root = query.run(|ctx| M::get_state_root(ctx).unwrap());
    assert_eq!(old_state_root, new_state_root);
    let old_state_root = new_state_root;

    // Insert existing key with different value.
    db.run(|ctx| {
        let mut table = ctx.get_table::<String, String>("data");

        table.insert("key1".to_string(), "other-value1".to_string());

        M::update_state_tree_from_context(ctx).unwrap();
    });

    // Check the state root hash.
    let new_state_root = query.run(|ctx| M::get_state_root(ctx).unwrap());
    assert_ne!(old_state_root, new_state_root);
    let old_state_root = new_state_root;

    // Remove non-existent key.
    db.run(|ctx| {
        let mut table = ctx.get_table::<String, String>("data");

        table.remove("unknown".to_string());

        M::update_state_tree_from_context(ctx).unwrap();
    });

    // Check the state root hash.
    let new_state_root = query.run(|ctx| M::get_state_root(ctx).unwrap());
    assert_eq!(old_state_root, new_state_root);

    // Verify the state tree by rebuilding it and comparing the root hashes.
    M::verify_state_tree(&mut db).unwrap();
}

#[test]
fn test_mpt_clear_and_rebuild_state_tree() {
    type S = DefaultSerdeBackend;
    type H = Sha256Hasher;
    type M = MptMerklizeProvider<InMemoryStorage, S, H>;

    let builder = AtomoBuilder::new(InMemoryStorage::default());
    let mut db = M::with_tables(builder.with_table::<String, String>("data"))
        .build()
        .unwrap();
    let query = db.query();

    // Insert a value.
    db.run(|ctx| {
        let mut table = ctx.get_table::<String, String>("data");

        table.insert("key1".to_string(), "value1".to_string());

        M::update_state_tree_from_context(ctx).unwrap();
    });

    // Get the state root hash.
    let state_root = query.run(|ctx| M::get_state_root(ctx).unwrap());

    // Rebuild the state tree.
    M::clear_and_rebuild_state_tree(&mut db).unwrap();

    // Check that the state root hash has not changed.
    let new_state_root = query.run(|ctx| M::get_state_root(ctx).unwrap());
    assert_eq!(state_root, new_state_root);
    let state_root = new_state_root;

    // Insert another value.
    db.run(|ctx| {
        let mut table = ctx.get_table::<String, String>("data");

        table.insert("key2".to_string(), "value2".to_string());

        M::update_state_tree_from_context(ctx).unwrap();
    });

    // Check that the state root hash has changed.
    let new_state_root = query.run(|ctx| M::get_state_root(ctx).unwrap());
    assert_ne!(state_root, new_state_root);
    let state_root = new_state_root;

    // Rebuild the state tree.
    M::clear_and_rebuild_state_tree(&mut db).unwrap();

    // Check that the state root hash has not changed.
    let new_state_root = query.run(|ctx| M::get_state_root(ctx).unwrap());
    assert_eq!(state_root, new_state_root);
}

#[test]
fn test_mpt_get_state_proof_of_membership() {
    type S = DefaultSerdeBackend;
    type H = Sha256Hasher;
    type M = MptMerklizeProvider<InMemoryStorage, S, H>;

    let builder = AtomoBuilder::new(InMemoryStorage::default());
    let mut db = M::with_tables(builder.with_table::<String, String>("data"))
        .build()
        .unwrap();
    let query = db.query();

    // Get a proof of non-membership with empty state, should fail.
    let state_root = query.run(|ctx| M::get_state_root(ctx).unwrap());
    let proof = query
        .run(|ctx| M::get_state_proof(ctx, "data", S::serialize(&"key1".to_string())))
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

        M::update_state_tree_from_context(ctx).unwrap();
    });

    // Get state root for proof verification.
    let state_root = query.run(|ctx| M::get_state_root(ctx).unwrap());

    // Get and verify proof of membership.
    let proof = query
        .run(|ctx| M::get_state_proof(ctx, "data", S::serialize(&"key1".to_string())).unwrap());
    proof
        .verify_membership::<String, String, M>(
            "data",
            "key1".to_string(),
            "value1".to_string(),
            state_root,
        )
        .unwrap();

    // Get and verify proof of non-membership of unknown key.
    let proof = query
        .run(|ctx| M::get_state_proof(ctx, "data", S::serialize(&"unknown".to_string())).unwrap());
    proof
        .verify_non_membership::<String, M>("data", "unknown".to_string(), state_root)
        .unwrap();

    // Remove a value.
    db.run(|ctx| {
        let mut table = ctx.get_table::<String, String>("data");

        table.remove("key2".to_string());

        M::update_state_tree_from_context(ctx).unwrap();
    });

    // Get state root for proof verification.
    let state_root = query.run(|ctx| M::get_state_root(ctx).unwrap());

    // Get and verify proof of non-membership of removed key.
    let proof = query
        .run(|ctx| M::get_state_proof(ctx, "data", S::serialize(&"key2".to_string())).unwrap());
    proof
        .verify_non_membership::<String, M>("data", "key2".to_string(), state_root)
        .unwrap();
}
