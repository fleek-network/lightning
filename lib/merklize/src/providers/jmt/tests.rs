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
use crate::providers::jmt::JmtMerklizeProvider;
use crate::{MerklizeProvider, MerklizedAtomo, StateRootHash};

#[test]
fn test_jmt_apply_state_tree_changes_with_updates() {
    type S = DefaultSerdeBackend;
    type H = Sha256Hasher;
    type M = JmtMerklizeProvider<InMemoryStorage, S, H>;

    let builder = AtomoBuilder::new(InMemoryStorage::default());
    let mut db = MerklizedAtomo::<_, InMemoryStorage, S, M>::new(
        M::with_tables(builder.with_table::<String, String>("data"))
            .build()
            .unwrap(),
    );

    // Check storage.
    {
        let storage = db.get_storage_backend_unsafe();
        assert_eq!(storage.keys(1).len(), 0); // nodes
        assert_eq!(storage.keys(2).len(), 0); // keys
    }

    // Insert a value.
    db.run(|ctx| {
        let mut table = ctx.get_table::<String, String>("data");

        table.insert("key1".to_string(), "value1".to_string());
    });

    // Check storage.
    {
        let storage = db.get_storage_backend_unsafe();
        assert_eq!(storage.keys(1).len(), 2); // nodes
        assert_eq!(storage.keys(2).len(), 1); // keys
    }

    // Insert another value.
    db.run(|ctx| {
        let mut table = ctx.get_table::<String, String>("data");

        table.insert("key2".to_string(), "value2".to_string());
    });

    // Check storage.
    {
        let storage = db.get_storage_backend_unsafe();
        assert_eq!(storage.keys(1).len(), 4); // nodes
        assert_eq!(storage.keys(2).len(), 2); // keys
    }

    // Insert another value.
    db.run(|ctx| {
        let mut table = ctx.get_table::<String, String>("data");

        table.insert("key3".to_string(), "value3".to_string());
    });

    // Check storage.
    {
        let storage = db.get_storage_backend_unsafe();
        assert_eq!(storage.keys(1).len(), 5); // nodes
        assert_eq!(storage.keys(2).len(), 3); // keys
    }

    // Remove a value.
    db.run(|ctx| {
        let mut table = ctx.get_table::<String, String>("data");

        table.remove("key2".to_string());
    });

    // Check storage.
    {
        let storage = db.get_storage_backend_unsafe();
        assert_eq!(storage.keys(1).len(), 4); // nodes
        assert_eq!(storage.keys(2).len(), 2); // keys
    }

    // Insert removed key with different value.
    db.run(|ctx| {
        let mut table = ctx.get_table::<String, String>("data");

        table.insert("key2".to_string(), "other-value2".to_string());
    });

    // Check storage.
    {
        let storage = db.get_storage_backend_unsafe();
        assert_eq!(storage.keys(1).len(), 5); // nodes
        assert_eq!(storage.keys(2).len(), 3); // keys
    }

    // Insert existing key with same value.
    db.run(|ctx| {
        let mut table = ctx.get_table::<String, String>("data");

        table.insert("key1".to_string(), "value1".to_string());
    });

    // Check storage.
    {
        let storage = db.get_storage_backend_unsafe();
        assert_eq!(storage.keys(1).len(), 5); // nodes
        assert_eq!(storage.keys(2).len(), 3); // keys
    }

    // Insert another value.
    db.run(|ctx| {
        let mut table = ctx.get_table::<String, String>("data");

        table.insert("key4".to_string(), "value4".to_string());
    });

    // Check storage.
    {
        let storage = db.get_storage_backend_unsafe();
        assert_eq!(storage.keys(1).len(), 8); // nodes
        assert_eq!(storage.keys(2).len(), 4); // keys
    }
}

#[test]
fn test_jmt_apply_state_tree_changes_with_no_changes() {
    type S = DefaultSerdeBackend;
    type H = Sha256Hasher;
    type M = JmtMerklizeProvider<InMemoryStorage, S, H>;

    let builder = AtomoBuilder::new(InMemoryStorage::default());
    let mut db = MerklizedAtomo::<_, InMemoryStorage, S, M>::new(
        M::with_tables(builder.with_table::<String, String>("data"))
            .build()
            .unwrap(),
    );

    // Check storage.
    {
        let storage = db.get_storage_backend_unsafe();
        assert_eq!(storage.keys(1).len(), 0); // nodes
        assert_eq!(storage.keys(2).len(), 0); // keys
    }

    // Open run context and apply state tree changes, but don't make any state changes before.
    db.run(|_ctx| {
        // Do nothing.
    });

    // Check storage.
    {
        let storage = db.get_storage_backend_unsafe();
        assert_eq!(storage.keys(1).len(), 2); // nodes
        assert_eq!(storage.keys(2).len(), 0); // keys
    }

    // Insert another value.
    db.run(|ctx| {
        let mut table = ctx.get_table::<String, String>("data");

        table.insert("key2".to_string(), "value2".to_string());
    });

    // Check storage.
    {
        let storage = db.get_storage_backend_unsafe();
        assert_eq!(storage.keys(1).len(), 2); // nodes
        assert_eq!(storage.keys(2).len(), 1); // keys
    }
}

#[test]
fn test_jmt_get_state_root_with_empty_state() {
    type S = DefaultSerdeBackend;
    type H = Sha256Hasher;
    type M = JmtMerklizeProvider<InMemoryStorage, S, H>;

    let builder = AtomoBuilder::new(InMemoryStorage::default());
    let db = MerklizedAtomo::<_, InMemoryStorage, S, M>::new(
        M::with_tables(builder.with_table::<String, String>("data"))
            .build()
            .unwrap(),
    );
    let query = db.query();

    let state_root = query.run(|ctx| M::get_state_root(ctx).unwrap());
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
    type M = JmtMerklizeProvider<InMemoryStorage, S, H>;

    let builder = AtomoBuilder::new(InMemoryStorage::default());
    let mut db = MerklizedAtomo::<_, B, S, M>::new(
        M::with_tables(builder.with_table::<String, String>("data"))
            .build()
            .unwrap(),
    );
    let query = db.query();

    fn assert_state_root_unchanged(
        query: &Atomo<QueryPerm, B, S>,
        old_state_root: StateRootHash,
    ) -> StateRootHash {
        let new_state_root = query.run(|ctx| M::get_state_root(ctx).unwrap());
        assert_eq!(old_state_root, new_state_root);
        new_state_root
    }

    fn assert_state_root_changed(
        query: &Atomo<QueryPerm, B, S>,
        old_state_root: StateRootHash,
    ) -> StateRootHash {
        let new_state_root = query.run(|ctx| M::get_state_root(ctx).unwrap());
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
    });
    let state_root = assert_state_root_changed(&query, state_root);

    // Insert another value and check that the state root has changed.
    db.run(|ctx| {
        let mut table = ctx.get_table::<String, String>("data");

        table.insert("key2".to_string(), "value2".to_string());
    });
    let state_root = assert_state_root_changed(&query, state_root);

    // Remove the inserted key and check that the state root has changed.
    db.run(|ctx| {
        let mut table = ctx.get_table::<String, String>("data");

        table.remove("key2".to_string());
    });
    let state_root = assert_state_root_changed(&query, state_root);

    // Insert removed key with different value and check that the state root has changed.
    db.run(|ctx| {
        let mut table = ctx.get_table::<String, String>("data");

        table.insert("key2".to_string(), "other-value2".to_string());
    });
    let state_root = assert_state_root_changed(&query, state_root);

    // Insert existing key with same value and check that the state root has not changed.
    db.run(|ctx| {
        let mut table = ctx.get_table::<String, String>("data");

        table.insert("key1".to_string(), "value1".to_string());
    });
    let state_root = assert_state_root_unchanged(&query, state_root);

    // Insert existing key with different value and check that the state root has not changed.
    db.run(|ctx| {
        let mut table = ctx.get_table::<String, String>("data");

        table.insert("key1".to_string(), "other-value1".to_string());
    });
    let state_root = assert_state_root_changed(&query, state_root);

    // Remove non-existent key and check that the state root has not changed.
    db.run(|ctx| {
        let mut table = ctx.get_table::<String, String>("data");

        table.remove("unknown".to_string());
    });
    assert_state_root_unchanged(&query, state_root);
}

#[test]
fn test_jmt_get_state_proof_of_membership() {
    type S = DefaultSerdeBackend;
    type H = Sha256Hasher;
    type M = JmtMerklizeProvider<InMemoryStorage, S, H>;

    let builder = AtomoBuilder::new(InMemoryStorage::default());
    let mut db = MerklizedAtomo::<_, InMemoryStorage, S, M>::new(
        M::with_tables(builder.with_table::<String, String>("data"))
            .build()
            .unwrap(),
    );
    let query = db.query();

    // Get a proof of non-membership with empty state, should fail.
    let res = query.run(|ctx| M::get_state_proof(ctx, "data", S::serialize(&"key1".to_string())));
    assert!(res.is_err());
    assert_eq!(
        res.err().unwrap().to_string(),
        "Cannot manufacture nonexistence proof by exclusion for the empty tree"
    );

    // Insert a value.
    db.run(|ctx| {
        let mut table = ctx.get_table::<String, String>("data");

        table.insert("key1".to_string(), "value1".to_string());
    });

    // Get state root for proof verification.
    let state_root = query.run(|ctx| M::get_state_root(ctx).unwrap());

    // Get and verify proof of membership.
    let proof = query
        .run(|ctx| M::get_state_proof(ctx, "data", S::serialize(&"key1".to_string())).unwrap());
    {
        let proof: ics23::CommitmentProof = proof.clone().into();
        assert!(matches!(
            proof.proof,
            Some(ics23::commitment_proof::Proof::Exist(_))
        ));
    }
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
    {
        let proof: ics23::CommitmentProof = proof.clone().into();
        assert!(matches!(
            proof.proof,
            Some(ics23::commitment_proof::Proof::Nonexist(_))
        ));
    }
    proof
        .verify_non_membership::<String, M>("data", "unknown".to_string(), state_root)
        .unwrap();

    // Remove a value.
    db.run(|ctx| {
        let mut table = ctx.get_table::<String, String>("data");

        table.remove("key2".to_string());
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
