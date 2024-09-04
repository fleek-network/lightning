use atomo::{
    Atomo,
    AtomoBuilder,
    DefaultSerdeBackend,
    InMemoryStorage,
    QueryPerm,
    SerdeBackend,
    StorageBackend,
};

use crate::hashers::sha2::Sha256Hasher;
use crate::proof::StateProof;
use crate::trees::jmt::JmtStateTree;
use crate::{StateRootHash, StateTree};

#[test]
fn test_jmt_update_state_tree_from_context_with_updates() {
    type S = DefaultSerdeBackend;
    type H = Sha256Hasher;
    type T = JmtStateTree<InMemoryStorage, S, H>;

    let builder = AtomoBuilder::new(InMemoryStorage::default());
    let mut db = T::register_tables(builder.with_table::<String, String>("data"))
        .build()
        .unwrap();

    // Check storage.
    {
        let storage = db.get_storage_backend_unsafe();
        assert_eq!(storage.keys(1).count(), 0); // nodes
        assert_eq!(storage.keys(2).count(), 0); // keys
    }

    // Insert a value.
    db.run(|ctx| {
        let mut table = ctx.get_table::<String, String>("data");

        table.insert("key1".to_string(), "value1".to_string());

        T::update_state_tree_from_context_changes(ctx).unwrap();
    });

    // Check storage.
    {
        let storage = db.get_storage_backend_unsafe();
        assert_eq!(storage.keys(1).count(), 2); // nodes
        assert_eq!(storage.keys(2).count(), 1); // keys
    }

    // Insert another value.
    db.run(|ctx| {
        let mut table = ctx.get_table::<String, String>("data");

        table.insert("key2".to_string(), "value2".to_string());

        T::update_state_tree_from_context_changes(ctx).unwrap();
    });

    // Check storage.
    {
        let storage = db.get_storage_backend_unsafe();
        assert_eq!(storage.keys(1).count(), 4); // nodes
        assert_eq!(storage.keys(2).count(), 2); // keys
    }

    // Insert another value.
    db.run(|ctx| {
        let mut table = ctx.get_table::<String, String>("data");

        table.insert("key3".to_string(), "value3".to_string());

        T::update_state_tree_from_context_changes(ctx).unwrap();
    });

    // Check storage.
    {
        let storage = db.get_storage_backend_unsafe();
        assert_eq!(storage.keys(1).count(), 5); // nodes
        assert_eq!(storage.keys(2).count(), 3); // keys
    }

    // Remove a value.
    db.run(|ctx| {
        let mut table = ctx.get_table::<String, String>("data");

        table.remove("key2".to_string());

        T::update_state_tree_from_context_changes(ctx).unwrap();
    });

    // Check storage.
    {
        let storage = db.get_storage_backend_unsafe();
        assert_eq!(storage.keys(1).count(), 4); // nodes
        assert_eq!(storage.keys(2).count(), 2); // keys
    }

    // Insert removed key with different value.
    db.run(|ctx| {
        let mut table = ctx.get_table::<String, String>("data");

        table.insert("key2".to_string(), "other-value2".to_string());

        T::update_state_tree_from_context_changes(ctx).unwrap();
    });

    // Check storage.
    {
        let storage = db.get_storage_backend_unsafe();
        assert_eq!(storage.keys(1).count(), 5); // nodes
        assert_eq!(storage.keys(2).count(), 3); // keys
    }

    // Insert existing key with same value.
    db.run(|ctx| {
        let mut table = ctx.get_table::<String, String>("data");

        table.insert("key1".to_string(), "value1".to_string());

        T::update_state_tree_from_context_changes(ctx).unwrap();
    });

    // Check storage.
    {
        let storage = db.get_storage_backend_unsafe();
        assert_eq!(storage.keys(1).count(), 5); // nodes
        assert_eq!(storage.keys(2).count(), 3); // keys
    }

    // Insert another value.
    db.run(|ctx| {
        let mut table = ctx.get_table::<String, String>("data");

        table.insert("key4".to_string(), "value4".to_string());

        T::update_state_tree_from_context_changes(ctx).unwrap();
    });

    // Check storage.
    {
        let storage = db.get_storage_backend_unsafe();
        assert_eq!(storage.keys(1).count(), 8); // nodes
        assert_eq!(storage.keys(2).count(), 4); // keys
    }
}

#[test]
fn test_jmt_update_state_tree_from_context_with_no_changes() {
    type S = DefaultSerdeBackend;
    type H = Sha256Hasher;
    type T = JmtStateTree<InMemoryStorage, S, H>;

    let _ = tracing_subscriber::fmt::try_init();

    let builder = AtomoBuilder::new(InMemoryStorage::default());
    let mut db = T::register_tables(builder.with_table::<String, String>("data"))
        .build()
        .unwrap();

    // Check storage.
    {
        let storage = db.get_storage_backend_unsafe();
        assert_eq!(storage.keys(1).count(), 0); // nodes
        assert_eq!(storage.keys(2).count(), 0); // keys
    }

    // Open run context and apply state tree changes, but don't make any state changes before.
    db.run(|ctx| {
        // Do nothing.

        T::update_state_tree_from_context_changes(ctx).unwrap();
    });

    // Check storage.
    {
        let storage = db.get_storage_backend_unsafe();
        assert_eq!(storage.keys(1).count(), 2); // nodes
        assert_eq!(storage.keys(2).count(), 0); // keys
    }

    // Insert another value.
    db.run(|ctx| {
        let mut table = ctx.get_table::<String, String>("data");

        table.insert("key2".to_string(), "value2".to_string());

        T::update_state_tree_from_context_changes(ctx).unwrap();
    });

    // Check storage.
    {
        let storage = db.get_storage_backend_unsafe();
        assert_eq!(storage.keys(1).count(), 2); // nodes
        assert_eq!(storage.keys(2).count(), 1); // keys
    }
}

#[test]
fn test_jmt_get_state_root_with_empty_state() {
    type S = DefaultSerdeBackend;
    type H = Sha256Hasher;
    type T = JmtStateTree<InMemoryStorage, S, H>;

    let builder = AtomoBuilder::new(InMemoryStorage::default());
    let db = T::register_tables(builder.with_table::<String, String>("data"))
        .build()
        .unwrap();
    let query = db.query();

    let state_root = query.run(|ctx| T::get_state_root(ctx).unwrap());
    assert_eq!(
        state_root,
        "5350415253455f4d45524b4c455f504c414345484f4c4445525f484153485f5f"
    );
}

#[test]
fn test_jmt_get_state_root_with_updates() {
    type S = DefaultSerdeBackend;
    type H = Sha256Hasher;
    type B = InMemoryStorage;
    type T = JmtStateTree<InMemoryStorage, S, H>;

    let builder = AtomoBuilder::new(InMemoryStorage::default());
    let mut db = T::register_tables(builder.with_table::<String, String>("data"))
        .build()
        .unwrap();
    let query = db.query();

    fn assert_state_root_unchanged(
        query: &Atomo<QueryPerm, B, S>,
        old_state_root: StateRootHash,
    ) -> StateRootHash {
        let new_state_root = query.run(|ctx| T::get_state_root(ctx).unwrap());
        assert_eq!(old_state_root, new_state_root);
        new_state_root
    }

    fn assert_state_root_changed(
        query: &Atomo<QueryPerm, B, S>,
        old_state_root: StateRootHash,
    ) -> StateRootHash {
        let new_state_root = query.run(|ctx| T::get_state_root(ctx).unwrap());
        assert_ne!(old_state_root, new_state_root);
        new_state_root
    }

    // Check that the state root is the empty tree root hash.
    let empty_state_root =
        StateRootHash::from_hex("5350415253455f4d45524b4c455f504c414345484f4c4445525f484153485f5f")
            .unwrap();
    let state_root = assert_state_root_unchanged(&query, empty_state_root);

    // Insert a value and check that the state root has changed.
    db.run(|ctx| {
        let mut table = ctx.get_table::<String, String>("data");

        table.insert("key1".to_string(), "value1".to_string());

        T::update_state_tree_from_context_changes(ctx).unwrap();
    });
    let state_root = assert_state_root_changed(&query, state_root);

    // Verify the state tree by rebuilding it and comparing the root hashes.
    T::verify_state_tree_unsafe(&mut db.query()).unwrap();

    // Insert another value and check that the state root has changed.
    db.run(|ctx| {
        let mut table = ctx.get_table::<String, String>("data");

        table.insert("key2".to_string(), "value2".to_string());

        T::update_state_tree_from_context_changes(ctx).unwrap();
    });
    let state_root = assert_state_root_changed(&query, state_root);

    // Remove the inserted key and check that the state root has changed.
    db.run(|ctx| {
        let mut table = ctx.get_table::<String, String>("data");

        table.remove("key2".to_string());

        T::update_state_tree_from_context_changes(ctx).unwrap();
    });
    let state_root = assert_state_root_changed(&query, state_root);

    // Verify the state tree by rebuilding it and comparing the root hashes.
    T::verify_state_tree_unsafe(&mut db.query()).unwrap();

    // Insert removed key with different value and check that the state root has changed.
    db.run(|ctx| {
        let mut table = ctx.get_table::<String, String>("data");

        table.insert("key2".to_string(), "other-value2".to_string());

        T::update_state_tree_from_context_changes(ctx).unwrap();
    });
    let state_root = assert_state_root_changed(&query, state_root);

    // Insert existing key with same value and check that the state root has not changed.
    db.run(|ctx| {
        let mut table = ctx.get_table::<String, String>("data");

        table.insert("key1".to_string(), "value1".to_string());

        T::update_state_tree_from_context_changes(ctx).unwrap();
    });
    let state_root = assert_state_root_unchanged(&query, state_root);

    // Insert existing key with different value and check that the state root has not changed.
    db.run(|ctx| {
        let mut table = ctx.get_table::<String, String>("data");

        table.insert("key1".to_string(), "other-value1".to_string());

        T::update_state_tree_from_context_changes(ctx).unwrap();
    });
    let state_root = assert_state_root_changed(&query, state_root);

    // Remove non-existent key and check that the state root has not changed.
    db.run(|ctx| {
        let mut table = ctx.get_table::<String, String>("data");

        table.remove("unknown".to_string());

        T::update_state_tree_from_context_changes(ctx).unwrap();
    });
    assert_state_root_unchanged(&query, state_root);

    // Verify the state tree by rebuilding it and comparing the root hashes.
    T::verify_state_tree_unsafe(&mut db.query()).unwrap();
}

#[test]
fn test_jmt_clear_and_rebuild_state_tree() {
    type S = DefaultSerdeBackend;
    type H = Sha256Hasher;
    type T = JmtStateTree<InMemoryStorage, S, H>;

    let builder = AtomoBuilder::new(InMemoryStorage::default());
    let mut db = T::register_tables(builder.with_table::<String, String>("data"))
        .build()
        .unwrap();
    let query = db.query();

    // Insert a value.
    db.run(|ctx| {
        let mut table = ctx.get_table::<String, String>("data");

        table.insert("key1".to_string(), "value1".to_string());

        T::update_state_tree_from_context_changes(ctx).unwrap();
    });

    // Get the state root hash.
    let state_root = query.run(|ctx| T::get_state_root(ctx).unwrap());

    // Rebuild the state tree.
    T::clear_and_rebuild_state_tree_unsafe(&mut db).unwrap();

    // Check that the state root hash has not changed.
    let new_state_root = query.run(|ctx| T::get_state_root(ctx).unwrap());
    assert_eq!(state_root, new_state_root);
    let state_root = new_state_root;

    // Insert another value.
    db.run(|ctx| {
        let mut table = ctx.get_table::<String, String>("data");

        table.insert("key2".to_string(), "value2".to_string());

        T::update_state_tree_from_context_changes(ctx).unwrap();
    });

    // Check that the state root hash has changed.
    let new_state_root = query.run(|ctx| T::get_state_root(ctx).unwrap());
    assert_ne!(state_root, new_state_root);
    let state_root = new_state_root;

    // Rebuild the state tree.
    T::clear_and_rebuild_state_tree_unsafe(&mut db).unwrap();

    // Check that the state root hash has not changed.
    let new_state_root = query.run(|ctx| T::get_state_root(ctx).unwrap());
    assert_eq!(state_root, new_state_root);
}

#[test]
fn test_jmt_get_state_proof_of_membership() {
    type S = DefaultSerdeBackend;
    type H = Sha256Hasher;
    type T = JmtStateTree<InMemoryStorage, S, H>;

    let builder = AtomoBuilder::new(InMemoryStorage::default());
    let mut db = T::register_tables(builder.with_table::<String, String>("data"))
        .build()
        .unwrap();
    let query = db.query();

    // Get a proof of non-membership with empty state, should fail.
    let res = query.run(|ctx| T::get_state_proof(ctx, "data", S::serialize(&"key1".to_string())));
    assert!(res.is_err());
    assert_eq!(
        res.err().unwrap().to_string(),
        "state tree error: Cannot manufacture nonexistence proof by exclusion for the empty tree"
    );

    // Insert a value.
    db.run(|ctx| {
        let mut table = ctx.get_table::<String, String>("data");

        table.insert("key1".to_string(), "value1".to_string());

        T::update_state_tree_from_context_changes(ctx).unwrap();
    });

    // Get state root for proof verification.
    let state_root = query.run(|ctx| T::get_state_root(ctx).unwrap());

    // Get and verify proof of membership.
    let proof = query
        .run(|ctx| T::get_state_proof(ctx, "data", S::serialize(&"key1".to_string())).unwrap());
    {
        let proof: ics23::CommitmentProof = proof.clone().into();
        assert!(matches!(
            proof.proof,
            Some(ics23::commitment_proof::Proof::Exist(_))
        ));
    }
    proof
        .verify_membership::<String, String, T>(
            "data",
            "key1".to_string(),
            "value1".to_string(),
            state_root,
        )
        .unwrap();

    // Get and verify proof of non-membership of unknown key.
    let proof = query
        .run(|ctx| T::get_state_proof(ctx, "data", S::serialize(&"unknown".to_string())).unwrap());
    {
        let proof: ics23::CommitmentProof = proof.clone().into();
        assert!(matches!(
            proof.proof,
            Some(ics23::commitment_proof::Proof::Nonexist(_))
        ));
    }
    proof
        .verify_non_membership::<String, T>("data", "unknown".to_string(), state_root)
        .unwrap();

    // Remove a value.
    db.run(|ctx| {
        let mut table = ctx.get_table::<String, String>("data");

        table.remove("key2".to_string());
    });

    // Get state root for proof verification.
    let state_root = query.run(|ctx| T::get_state_root(ctx).unwrap());

    // Get and verify proof of non-membership of removed key.
    let proof = query
        .run(|ctx| T::get_state_proof(ctx, "data", S::serialize(&"key2".to_string())).unwrap());
    proof
        .verify_non_membership::<String, T>("data", "key2".to_string(), state_root)
        .unwrap();
}
