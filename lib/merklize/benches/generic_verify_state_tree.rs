#![feature(test)]
extern crate test;

mod generic_utils;

use atomo::{AtomoBuilder, DefaultSerdeBackend, StorageBackendConstructor};
use generic_utils::{rocksdb_builder, DATA_COUNT_COMPLEX, DATA_COUNT_MEDIUM, DATA_COUNT_SIMPLE};
use merklize::hashers::blake3::Blake3Hasher;
use merklize::hashers::keccak::KeccakHasher;
use merklize::hashers::sha2::Sha256Hasher;
use merklize::providers::jmt::JmtMerklizeProvider;
use merklize::providers::mpt::MptMerklizeProvider;
use merklize::MerklizeProvider;
use tempfile::tempdir;
use test::Bencher;

// JMT

type Jmt<B, H> = JmtMerklizeProvider<B, DefaultSerdeBackend, H>;

#[bench]
fn bench_generic_verify_state_tree_rocksdb_jmt_keccak256_simple(b: &mut Bencher) {
    let temp_dir = tempdir().unwrap();
    generic_bench_verify_state_tree::<_, Jmt<_, KeccakHasher>>(
        b,
        rocksdb_builder(&temp_dir),
        DATA_COUNT_SIMPLE,
    );
}

#[bench]
fn bench_generic_verify_state_tree_rocksdb_jmt_blake3_simple(b: &mut Bencher) {
    let temp_dir = tempdir().unwrap();
    generic_bench_verify_state_tree::<_, Jmt<_, Blake3Hasher>>(
        b,
        rocksdb_builder(&temp_dir),
        DATA_COUNT_SIMPLE,
    );
}

#[bench]
fn bench_generic_verify_state_tree_rocksdb_jmt_sha256_simple(b: &mut Bencher) {
    let temp_dir = tempdir().unwrap();
    generic_bench_verify_state_tree::<_, Jmt<_, Sha256Hasher>>(
        b,
        rocksdb_builder(&temp_dir),
        DATA_COUNT_SIMPLE,
    );
}

#[bench]
fn bench_generic_verify_state_tree_rocksdb_jmt_keccak256_medium(b: &mut Bencher) {
    let temp_dir = tempdir().unwrap();
    generic_bench_verify_state_tree::<_, Jmt<_, KeccakHasher>>(
        b,
        rocksdb_builder(&temp_dir),
        DATA_COUNT_MEDIUM,
    );
}

#[bench]
fn bench_generic_verify_state_tree_rocksdb_jmt_blake3_medium(b: &mut Bencher) {
    let temp_dir = tempdir().unwrap();
    generic_bench_verify_state_tree::<_, Jmt<_, Blake3Hasher>>(
        b,
        rocksdb_builder(&temp_dir),
        DATA_COUNT_MEDIUM,
    );
}

#[bench]
fn bench_generic_verify_state_tree_rocksdb_jmt_sha256_medium(b: &mut Bencher) {
    let temp_dir = tempdir().unwrap();
    generic_bench_verify_state_tree::<_, Jmt<_, Sha256Hasher>>(
        b,
        rocksdb_builder(&temp_dir),
        DATA_COUNT_MEDIUM,
    );
}

#[bench]
fn bench_generic_verify_state_tree_rocksdb_jmt_keccak256_complex(b: &mut Bencher) {
    let temp_dir = tempdir().unwrap();
    generic_bench_verify_state_tree::<_, Jmt<_, KeccakHasher>>(
        b,
        rocksdb_builder(&temp_dir),
        DATA_COUNT_COMPLEX,
    );
}

#[bench]
fn bench_generic_verify_state_tree_rocksdb_jmt_blake3_complex(b: &mut Bencher) {
    let temp_dir = tempdir().unwrap();
    generic_bench_verify_state_tree::<_, Jmt<_, Blake3Hasher>>(
        b,
        rocksdb_builder(&temp_dir),
        DATA_COUNT_COMPLEX,
    );
}

#[bench]
fn bench_generic_verify_state_tree_rocksdb_jmt_sha256_complex(b: &mut Bencher) {
    let temp_dir = tempdir().unwrap();
    generic_bench_verify_state_tree::<_, Jmt<_, Sha256Hasher>>(
        b,
        rocksdb_builder(&temp_dir),
        DATA_COUNT_COMPLEX,
    );
}

// MPT

type Mpt<B, H> = MptMerklizeProvider<B, DefaultSerdeBackend, H>;

#[bench]
fn bench_generic_verify_state_tree_rocksdb_mpt_keccak256_simple(b: &mut Bencher) {
    let temp_dir = tempdir().unwrap();
    generic_bench_verify_state_tree::<_, Mpt<_, KeccakHasher>>(
        b,
        rocksdb_builder(&temp_dir),
        DATA_COUNT_SIMPLE,
    );
}

#[bench]
fn bench_generic_verify_state_tree_rocksdb_mpt_blake3_simple(b: &mut Bencher) {
    let temp_dir = tempdir().unwrap();
    generic_bench_verify_state_tree::<_, Mpt<_, Blake3Hasher>>(
        b,
        rocksdb_builder(&temp_dir),
        DATA_COUNT_SIMPLE,
    );
}

#[bench]
fn bench_generic_verify_state_tree_rocksdb_mpt_sha256_simple(b: &mut Bencher) {
    let temp_dir = tempdir().unwrap();
    generic_bench_verify_state_tree::<_, Mpt<_, Sha256Hasher>>(
        b,
        rocksdb_builder(&temp_dir),
        DATA_COUNT_SIMPLE,
    );
}

#[bench]
fn bench_generic_verify_state_tree_rocksdb_mpt_keccak256_medium(b: &mut Bencher) {
    let temp_dir = tempdir().unwrap();
    generic_bench_verify_state_tree::<_, Mpt<_, KeccakHasher>>(
        b,
        rocksdb_builder(&temp_dir),
        DATA_COUNT_MEDIUM,
    );
}

#[bench]
fn bench_generic_verify_state_tree_rocksdb_mpt_blake3_medium(b: &mut Bencher) {
    let temp_dir = tempdir().unwrap();
    generic_bench_verify_state_tree::<_, Mpt<_, Blake3Hasher>>(
        b,
        rocksdb_builder(&temp_dir),
        DATA_COUNT_MEDIUM,
    );
}

#[bench]
fn bench_generic_verify_state_tree_rocksdb_mpt_sha256_medium(b: &mut Bencher) {
    let temp_dir = tempdir().unwrap();
    generic_bench_verify_state_tree::<_, Mpt<_, Sha256Hasher>>(
        b,
        rocksdb_builder(&temp_dir),
        DATA_COUNT_MEDIUM,
    );
}

#[bench]
fn bench_generic_verify_state_tree_rocksdb_mpt_keccak256_complex(b: &mut Bencher) {
    let temp_dir = tempdir().unwrap();
    generic_bench_verify_state_tree::<_, Mpt<_, KeccakHasher>>(
        b,
        rocksdb_builder(&temp_dir),
        DATA_COUNT_COMPLEX,
    );
}

#[bench]
fn bench_generic_verify_state_tree_rocksdb_mpt_blake3_complex(b: &mut Bencher) {
    let temp_dir = tempdir().unwrap();
    generic_bench_verify_state_tree::<_, Mpt<_, Blake3Hasher>>(
        b,
        rocksdb_builder(&temp_dir),
        DATA_COUNT_COMPLEX,
    );
}

#[bench]
fn bench_generic_verify_state_tree_rocksdb_mpt_sha256_complex(b: &mut Bencher) {
    let temp_dir = tempdir().unwrap();
    generic_bench_verify_state_tree::<_, Mpt<_, Sha256Hasher>>(
        b,
        rocksdb_builder(&temp_dir),
        DATA_COUNT_COMPLEX,
    );
}

fn generic_bench_verify_state_tree<C: StorageBackendConstructor, M>(
    b: &mut Bencher,
    builder: C,
    data_count: usize,
) where
    M: MerklizeProvider<Storage = C::Storage>,
{
    let mut db = M::with_tables(AtomoBuilder::new(builder).with_table::<String, String>("data"))
        .build()
        .unwrap();

    db.run(|ctx| {
        let mut data_table = ctx.get_table::<String, String>("data");

        for i in 1..=data_count {
            data_table.insert(format!("key{i}"), format!("value{i}"));
        }

        M::update_state_tree_from_context(ctx).unwrap();
    });

    b.iter(|| {
        M::verify_state_tree(&mut db).unwrap();
    })
}
