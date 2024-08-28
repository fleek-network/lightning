#![feature(test)]
extern crate test;

mod generic_utils;

use atomo::{AtomoBuilder, DefaultSerdeBackend, StorageBackendConstructor};
use generic_utils::{rocksdb_builder, DATA_COUNT_COMPLEX, DATA_COUNT_MEDIUM, DATA_COUNT_SIMPLE};
use merklize::hashers::blake3::Blake3Hasher;
use merklize::hashers::keccak::KeccakHasher;
use merklize::hashers::sha2::Sha256Hasher;
use merklize::trees::jmt::JmtStateTree;
use merklize::trees::mpt::MptStateTree;
use merklize::StateTree;
use tempfile::tempdir;
use test::Bencher;

// Baseline

#[bench]
fn bench_generic_commit_changes_rocksdb_baseline_simple(b: &mut Bencher) {
    let temp_dir = tempdir().unwrap();
    generic_baseline_bench_commit_changes(b, rocksdb_builder(&temp_dir), DATA_COUNT_SIMPLE);
}

#[bench]
fn bench_generic_commit_changes_rocksdb_baseline_medium(b: &mut Bencher) {
    let temp_dir = tempdir().unwrap();
    generic_baseline_bench_commit_changes(b, rocksdb_builder(&temp_dir), DATA_COUNT_MEDIUM);
}

#[bench]
fn bench_generic_commit_changes_rocksdb_baseline_complex(b: &mut Bencher) {
    let temp_dir = tempdir().unwrap();
    generic_baseline_bench_commit_changes(b, rocksdb_builder(&temp_dir), DATA_COUNT_COMPLEX);
}

// JMT

#[bench]
fn bench_generic_commit_changes_rocksdb_jmt_keccak256_simple(b: &mut Bencher) {
    let temp_dir = tempdir().unwrap();
    generic_merklize_bench_commit_changes::<_, JmtStateTree<_, DefaultSerdeBackend, KeccakHasher>>(
        b,
        rocksdb_builder(&temp_dir),
        DATA_COUNT_SIMPLE,
    );
}

#[bench]
fn bench_generic_commit_changes_rocksdb_jmt_blake3_simple(b: &mut Bencher) {
    let temp_dir = tempdir().unwrap();
    generic_merklize_bench_commit_changes::<_, JmtStateTree<_, DefaultSerdeBackend, Blake3Hasher>>(
        b,
        rocksdb_builder(&temp_dir),
        DATA_COUNT_SIMPLE,
    );
}

#[bench]
fn bench_generic_commit_changes_rocksdb_jmt_sha256_simple(b: &mut Bencher) {
    let temp_dir = tempdir().unwrap();
    generic_merklize_bench_commit_changes::<_, JmtStateTree<_, DefaultSerdeBackend, Sha256Hasher>>(
        b,
        rocksdb_builder(&temp_dir),
        DATA_COUNT_SIMPLE,
    );
}

#[bench]
fn bench_generic_commit_changes_rocksdb_jmt_keccak256_medium(b: &mut Bencher) {
    let temp_dir = tempdir().unwrap();
    generic_merklize_bench_commit_changes::<_, JmtStateTree<_, DefaultSerdeBackend, KeccakHasher>>(
        b,
        rocksdb_builder(&temp_dir),
        DATA_COUNT_MEDIUM,
    );
}

#[bench]
fn bench_generic_commit_changes_rocksdb_jmt_blake3_medium(b: &mut Bencher) {
    let temp_dir = tempdir().unwrap();
    generic_merklize_bench_commit_changes::<_, JmtStateTree<_, DefaultSerdeBackend, Blake3Hasher>>(
        b,
        rocksdb_builder(&temp_dir),
        DATA_COUNT_MEDIUM,
    );
}

#[bench]
fn bench_generic_commit_changes_rocksdb_jmt_sha256_medium(b: &mut Bencher) {
    let temp_dir = tempdir().unwrap();
    generic_merklize_bench_commit_changes::<_, JmtStateTree<_, DefaultSerdeBackend, Sha256Hasher>>(
        b,
        rocksdb_builder(&temp_dir),
        DATA_COUNT_MEDIUM,
    );
}

#[bench]
fn bench_generic_commit_changes_rocksdb_jmt_keccak256_complex(b: &mut Bencher) {
    let temp_dir = tempdir().unwrap();
    generic_merklize_bench_commit_changes::<_, JmtStateTree<_, DefaultSerdeBackend, KeccakHasher>>(
        b,
        rocksdb_builder(&temp_dir),
        DATA_COUNT_COMPLEX,
    );
}

#[bench]
fn bench_generic_commit_changes_rocksdb_jmt_blake3_complex(b: &mut Bencher) {
    let temp_dir = tempdir().unwrap();
    generic_merklize_bench_commit_changes::<_, JmtStateTree<_, DefaultSerdeBackend, Blake3Hasher>>(
        b,
        rocksdb_builder(&temp_dir),
        DATA_COUNT_COMPLEX,
    );
}

#[bench]
fn bench_generic_commit_changes_rocksdb_jmt_sha256_complex(b: &mut Bencher) {
    let temp_dir = tempdir().unwrap();
    generic_merklize_bench_commit_changes::<_, JmtStateTree<_, DefaultSerdeBackend, Sha256Hasher>>(
        b,
        rocksdb_builder(&temp_dir),
        DATA_COUNT_COMPLEX,
    );
}

// MPT

#[bench]
fn bench_generic_commit_changes_rocksdb_mpt_keccak256_simple(b: &mut Bencher) {
    let temp_dir = tempdir().unwrap();
    generic_merklize_bench_commit_changes::<_, MptStateTree<_, DefaultSerdeBackend, KeccakHasher>>(
        b,
        rocksdb_builder(&temp_dir),
        DATA_COUNT_SIMPLE,
    );
}

#[bench]
fn bench_generic_commit_changes_rocksdb_mpt_blake3_simple(b: &mut Bencher) {
    let temp_dir = tempdir().unwrap();
    generic_merklize_bench_commit_changes::<_, MptStateTree<_, DefaultSerdeBackend, Blake3Hasher>>(
        b,
        rocksdb_builder(&temp_dir),
        DATA_COUNT_SIMPLE,
    );
}

#[bench]
fn bench_generic_commit_changes_rocksdb_mpt_sha256_simple(b: &mut Bencher) {
    let temp_dir = tempdir().unwrap();
    generic_merklize_bench_commit_changes::<_, MptStateTree<_, DefaultSerdeBackend, Sha256Hasher>>(
        b,
        rocksdb_builder(&temp_dir),
        DATA_COUNT_SIMPLE,
    );
}

#[bench]
fn bench_generic_commit_changes_rocksdb_mpt_keccak256_medium(b: &mut Bencher) {
    let temp_dir = tempdir().unwrap();
    generic_merklize_bench_commit_changes::<_, MptStateTree<_, DefaultSerdeBackend, KeccakHasher>>(
        b,
        rocksdb_builder(&temp_dir),
        DATA_COUNT_MEDIUM,
    );
}

#[bench]
fn bench_generic_commit_changes_rocksdb_mpt_blake3_medium(b: &mut Bencher) {
    let temp_dir = tempdir().unwrap();
    generic_merklize_bench_commit_changes::<_, MptStateTree<_, DefaultSerdeBackend, Blake3Hasher>>(
        b,
        rocksdb_builder(&temp_dir),
        DATA_COUNT_MEDIUM,
    );
}

#[bench]
fn bench_generic_commit_changes_rocksdb_mpt_sha256_medium(b: &mut Bencher) {
    let temp_dir = tempdir().unwrap();
    generic_merklize_bench_commit_changes::<_, MptStateTree<_, DefaultSerdeBackend, Sha256Hasher>>(
        b,
        rocksdb_builder(&temp_dir),
        DATA_COUNT_MEDIUM,
    );
}

#[bench]
fn bench_generic_commit_changes_rocksdb_mpt_keccak256_complex(b: &mut Bencher) {
    let temp_dir = tempdir().unwrap();
    generic_merklize_bench_commit_changes::<_, MptStateTree<_, DefaultSerdeBackend, KeccakHasher>>(
        b,
        rocksdb_builder(&temp_dir),
        DATA_COUNT_COMPLEX,
    );
}

#[bench]
fn bench_generic_commit_changes_rocksdb_mpt_blake3_complex(b: &mut Bencher) {
    let temp_dir = tempdir().unwrap();
    generic_merklize_bench_commit_changes::<_, MptStateTree<_, DefaultSerdeBackend, Blake3Hasher>>(
        b,
        rocksdb_builder(&temp_dir),
        DATA_COUNT_COMPLEX,
    );
}

#[bench]
fn bench_generic_commit_changes_rocksdb_mpt_sha256_complex(b: &mut Bencher) {
    let temp_dir = tempdir().unwrap();
    generic_merklize_bench_commit_changes::<_, MptStateTree<_, DefaultSerdeBackend, Sha256Hasher>>(
        b,
        rocksdb_builder(&temp_dir),
        DATA_COUNT_COMPLEX,
    );
}

fn generic_baseline_bench_commit_changes<C: StorageBackendConstructor>(
    b: &mut Bencher,
    builder: C,
    data_count: usize,
) {
    let mut db = AtomoBuilder::<C>::new(builder)
        .with_table::<String, String>("data")
        .build()
        .unwrap();

    b.iter(|| {
        db.run(|ctx| {
            let mut data_table = ctx.get_table::<String, String>("data");

            for i in 1..=data_count {
                data_table.insert(format!("key{i}"), format!("value{i}"));
            }
        });
    })
}

fn generic_merklize_bench_commit_changes<C: StorageBackendConstructor, T>(
    b: &mut Bencher,
    builder: C,
    data_count: usize,
) where
    T: StateTree<Storage = C::Storage>,
{
    let mut db =
        T::register_tables(AtomoBuilder::new(builder).with_table::<String, String>("data"))
            .build()
            .unwrap();

    b.iter(|| {
        db.run(|ctx| {
            let mut data_table = ctx.get_table::<String, String>("data");

            for i in 1..=data_count {
                data_table.insert(format!("key{i}"), format!("value{i}"));
            }

            T::update_state_tree_from_context_changes(ctx).unwrap();
        });
    })
}
