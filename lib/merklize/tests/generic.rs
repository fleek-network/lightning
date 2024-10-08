use atomo::{
    AtomoBuilder,
    DefaultSerdeBackend,
    InMemoryStorage,
    SerdeBackend,
    StorageBackendConstructor,
};
use atomo_rocks::{Options, RocksBackendBuilder};
use merklize::hashers::blake3::Blake3Hasher;
use merklize::hashers::keccak::KeccakHasher;
use merklize::hashers::sha2::Sha256Hasher;
use merklize::trees::jmt::JmtStateTree;
use merklize::trees::mpt::MptStateTree;
use merklize::{StateProof, StateRootHash, StateTree};
use tempfile::tempdir;

// JMT

#[test]
fn test_generic_jmt_memdb_sha256() {
    let builder = InMemoryStorage::default();
    test_generic::<_, DefaultSerdeBackend, JmtStateTree<_, _, Sha256Hasher>>(builder);
}

#[test]
fn test_generic_jmt_rocksdb_sha256() {
    let temp_dir = tempdir().unwrap();
    let mut options = Options::default();
    options.create_if_missing(true);
    options.create_missing_column_families(true);
    let builder = RocksBackendBuilder::new(temp_dir.path()).with_options(options);
    test_generic::<_, DefaultSerdeBackend, JmtStateTree<_, _, Sha256Hasher>>(builder);
}

#[test]
fn test_generic_jmt_memdb_keccak256() {
    let builder = InMemoryStorage::default();
    test_generic::<_, DefaultSerdeBackend, JmtStateTree<_, _, KeccakHasher>>(builder);
}

#[test]
fn test_generic_jmt_rocksdb_keccak256() {
    let temp_dir = tempdir().unwrap();
    let mut options = Options::default();
    options.create_if_missing(true);
    options.create_missing_column_families(true);
    let builder = RocksBackendBuilder::new(temp_dir.path()).with_options(options);
    test_generic::<_, DefaultSerdeBackend, JmtStateTree<_, _, KeccakHasher>>(builder);
}

#[test]
fn test_generic_jmt_memdb_blake3() {
    let builder = InMemoryStorage::default();
    test_generic::<_, DefaultSerdeBackend, JmtStateTree<_, _, Blake3Hasher>>(builder);
}

#[test]
fn test_generic_jmt_rocksdb_blake3() {
    let temp_dir = tempdir().unwrap();
    let mut options = Options::default();
    options.create_if_missing(true);
    options.create_missing_column_families(true);
    let builder = RocksBackendBuilder::new(temp_dir.path()).with_options(options);
    test_generic::<_, DefaultSerdeBackend, JmtStateTree<_, _, Blake3Hasher>>(builder);
}

// MPT

#[test]
fn test_generic_mpt_memdb_sha256() {
    let builder = InMemoryStorage::default();
    test_generic::<_, DefaultSerdeBackend, MptStateTree<_, _, Sha256Hasher>>(builder);
}

#[test]
fn test_generic_mpt_rocksdb_sha256() {
    let temp_dir = tempdir().unwrap();
    let mut options = Options::default();
    options.create_if_missing(true);
    options.create_missing_column_families(true);
    let builder = RocksBackendBuilder::new(temp_dir.path()).with_options(options);
    test_generic::<_, DefaultSerdeBackend, MptStateTree<_, _, Sha256Hasher>>(builder);
}

#[test]
fn test_generic_mpt_memdb_keccak256() {
    let builder = InMemoryStorage::default();
    test_generic::<_, DefaultSerdeBackend, MptStateTree<_, _, KeccakHasher>>(builder);
}

#[test]
fn test_generic_mpt_rocksdb_keccak256() {
    let temp_dir = tempdir().unwrap();
    let mut options = Options::default();
    options.create_if_missing(true);
    options.create_missing_column_families(true);
    let builder = RocksBackendBuilder::new(temp_dir.path()).with_options(options);
    test_generic::<_, DefaultSerdeBackend, MptStateTree<_, _, KeccakHasher>>(builder);
}

#[test]
fn test_generic_mpt_memdb_blake3() {
    let builder = InMemoryStorage::default();
    test_generic::<_, DefaultSerdeBackend, MptStateTree<_, _, Blake3Hasher>>(builder);
}

#[test]
fn test_generic_mpt_rocksdb_blake3() {
    let temp_dir = tempdir().unwrap();
    let mut options = Options::default();
    options.create_if_missing(true);
    options.create_missing_column_families(true);
    let builder = RocksBackendBuilder::new(temp_dir.path()).with_options(options);
    test_generic::<_, DefaultSerdeBackend, MptStateTree<_, _, Blake3Hasher>>(builder);
}

fn test_generic<
    C: StorageBackendConstructor,
    S: SerdeBackend,
    T: StateTree<Storage = C::Storage, Serde = S>,
>(
    builder: C,
) {
    let builder = T::register_tables(
        AtomoBuilder::new(builder)
            .with_table::<String, String>("data")
            .enable_iter("data")
            .with_table::<u8, u8>("other"),
    );
    let mut db = builder.build().unwrap();
    let query = db.query();

    // Check that the state tree is empty.
    assert!(T::is_empty_state_tree_unsafe(&mut db.query()).unwrap());

    // Check state root.
    let initial_state_root = query.run(|ctx| T::get_state_root(ctx).unwrap());
    let mut old_state_root = initial_state_root;

    // Check that the state tree is still empty.
    assert!(T::is_empty_state_tree_unsafe(&mut db.query()).unwrap());

    // Insert initial data.
    let data_insert_count = 10;
    db.run(|ctx: _| {
        let mut data_table = ctx.get_table::<String, String>("data");

        for i in 1..=data_insert_count {
            data_table.insert(format!("key{i}"), format!("value{i}"));
        }

        T::update_state_tree_from_context_changes(ctx).unwrap();
    });

    // Check that the state tree is not empty.
    assert!(!T::is_empty_state_tree_unsafe(&mut db.query()).unwrap());

    // Check data via reader.
    query.run(|ctx| {
        let data_table = ctx.get_table::<String, String>("data");

        // Check state root.
        let new_state_root = T::get_state_root(ctx).unwrap();
        assert_ne!(new_state_root, old_state_root);
        assert_ne!(new_state_root, StateRootHash::default());
        old_state_root = new_state_root;

        // Check data key count.
        let keys = data_table.keys().collect::<Vec<_>>();
        assert_eq!(keys.len(), data_insert_count);

        // Check data values for each key.
        for i in 1..=data_insert_count {
            assert_eq!(data_table.get(format!("key{i}")), Some(format!("value{i}")));
        }

        // Check existence proofs.
        for i in 1..=data_insert_count {
            // Generate proof.
            let proof = T::get_state_proof(
                ctx,
                "data",
                T::Serde::serialize::<Vec<u8>>(&format!("key{i}").as_bytes().to_vec()),
            )
            .unwrap();

            // Verify proof.
            proof
                .verify_membership::<String, String, T>(
                    "data",
                    format!("key{i}").to_string(),
                    format!("value{i}").to_string(),
                    new_state_root,
                )
                .unwrap();
        }

        // Check non-existence proof.
        let proof = T::get_state_proof(ctx, "data", S::serialize(&"unknown".to_string())).unwrap();
        proof
            .verify_non_membership::<String, T>("data", "unknown".to_string(), new_state_root)
            .unwrap();
    });

    // Verify state tree.
    T::verify_state_tree_unsafe(&mut db.query()).unwrap();

    // Insert more data.
    db.run(|ctx: _| {
        let mut data_table = ctx.get_table::<String, String>("data");

        for i in 1..=data_insert_count {
            data_table.insert(format!("other{i}"), format!("value{i}"));
        }

        T::update_state_tree_from_context_changes(ctx).unwrap();
    });

    // Check state root.
    let new_state_root = query.run(|ctx| T::get_state_root(ctx).unwrap());
    assert_ne!(new_state_root, old_state_root);
    assert_ne!(new_state_root, StateRootHash::default());
    let old_state_root = new_state_root;

    // Check that the state tree is not empty.
    assert!(!T::is_empty_state_tree_unsafe(&mut db.query()).unwrap());

    // Verify state tree.
    T::verify_state_tree_unsafe(&mut db.query()).unwrap();

    // Remove some data.
    db.run(|ctx: _| {
        let mut data_table = ctx.get_table::<String, String>("data");

        data_table.remove("key3".to_string());
        data_table.remove("other5".to_string());
        data_table.remove("other9".to_string());

        T::update_state_tree_from_context_changes(ctx).unwrap();
    });

    // Check state root.
    let new_state_root = query.run(|ctx| T::get_state_root(ctx).unwrap());
    assert_ne!(new_state_root, old_state_root);
    assert_ne!(new_state_root, StateRootHash::default());

    // Verify state tree.
    T::verify_state_tree_unsafe(&mut db.query()).unwrap();

    // Check that the state tree is not empty.
    assert!(!T::is_empty_state_tree_unsafe(&mut db.query()).unwrap());

    // Check non-membership proofs for removed data.
    query.run(|ctx| {
        // Check non-existence proof for key3.
        let proof = T::get_state_proof(ctx, "data", S::serialize(&"key3".to_string())).unwrap();
        proof
            .verify_non_membership::<String, T>("data", "key3".to_string(), new_state_root)
            .unwrap();

        // Check non-existence proof for other5.
        let proof = T::get_state_proof(ctx, "data", S::serialize(&"other5".to_string())).unwrap();
        proof
            .verify_non_membership::<String, T>("data", "other5".to_string(), new_state_root)
            .unwrap();

        // Check non-existence proof for other9.
        let proof = T::get_state_proof(ctx, "data", S::serialize(&"other9".to_string())).unwrap();
        proof
            .verify_non_membership::<String, T>("data", "other9".to_string(), new_state_root)
            .unwrap();
    });

    // Clear and rebuild state tree.
    T::clear_and_rebuild_state_tree_unsafe(&mut db).unwrap();

    // Verify state tree.
    T::verify_state_tree_unsafe(&mut db.query()).unwrap();

    // Check that the state tree is not empty.
    assert!(!T::is_empty_state_tree_unsafe(&mut db.query()).unwrap());
}
