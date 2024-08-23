#![feature(test)]
extern crate test;

use atomo::DefaultSerdeBackend;
use futures::executor::block_on;
use lightning_application::storage::AtomoStorage;
use lightning_test_utils::application::{
    create_rocksdb_env,
    new_complex_block,
    new_medium_block,
    new_simple_block,
    BaselineMerklizeProvider,
    DummyPutter,
};
use merklize::hashers::blake3::Blake3Hasher;
use merklize::hashers::keccak::KeccakHasher;
use merklize::hashers::sha2::Sha256Hasher;
use merklize::providers::jmt::JmtMerklizeProvider;
use merklize::providers::mpt::MptMerklizeProvider;
use tempfile::tempdir;
use test::Bencher;

// Baseline

#[bench]
fn bench_application_commit_changes_rocksdb_baseline_simple(b: &mut Bencher) {
    let temp_dir = tempdir().unwrap();
    let mut env = create_rocksdb_env::<BaselineMerklizeProvider>(&temp_dir);
    let (block, _stake_amount, _eth_addresses) = new_simple_block();
    b.iter(|| block_on(env.run(block.clone(), || DummyPutter {})))
}

#[bench]
fn bench_application_commit_changes_rocksdb_baseline_medium(b: &mut Bencher) {
    let temp_dir = tempdir().unwrap();
    let mut env = create_rocksdb_env::<BaselineMerklizeProvider>(&temp_dir);
    let (block, _stake_amount, _eth_addresses) = new_medium_block();
    b.iter(|| block_on(env.run(block.clone(), || DummyPutter {})))
}

#[bench]
fn bench_application_commit_changes_rocksdb_baseline_complex(b: &mut Bencher) {
    let temp_dir = tempdir().unwrap();
    let mut env = create_rocksdb_env::<BaselineMerklizeProvider>(&temp_dir);
    let (block, _stake_amount, _eth_addresses, _node_public_keys) = new_complex_block();
    b.iter(|| block_on(env.run(block.clone(), || DummyPutter {})))
}

// JMT

#[bench]
fn bench_application_commit_changes_rocksdb_jmt_blake3_simple(b: &mut Bencher) {
    let temp_dir = tempdir().unwrap();
    let mut env = create_rocksdb_env::<
        JmtMerklizeProvider<AtomoStorage, DefaultSerdeBackend, Blake3Hasher>,
    >(&temp_dir);
    let (block, _stake_amount, _eth_addresses) = new_simple_block();
    b.iter(|| block_on(env.run(block.clone(), || DummyPutter {})))
}

#[bench]
fn bench_application_commit_changes_rocksdb_jmt_blake3_medium(b: &mut Bencher) {
    let temp_dir = tempdir().unwrap();
    let mut env = create_rocksdb_env::<
        JmtMerklizeProvider<AtomoStorage, DefaultSerdeBackend, Blake3Hasher>,
    >(&temp_dir);
    let (block, _stake_amount, _eth_addresses) = new_medium_block();
    b.iter(|| block_on(env.run(block.clone(), || DummyPutter {})))
}

#[bench]
fn bench_application_commit_changes_rocksdb_jmt_blake3_complex(b: &mut Bencher) {
    let temp_dir = tempdir().unwrap();
    let mut env = create_rocksdb_env::<
        JmtMerklizeProvider<AtomoStorage, DefaultSerdeBackend, Blake3Hasher>,
    >(&temp_dir);
    let (block, _stake_amount, _eth_addresses, _node_public_keys) = new_complex_block();
    b.iter(|| block_on(env.run(block.clone(), || DummyPutter {})))
}

#[bench]
fn bench_application_commit_changes_rocksdb_jmt_keccak256_simple(b: &mut Bencher) {
    let temp_dir = tempdir().unwrap();
    let mut env = create_rocksdb_env::<
        JmtMerklizeProvider<AtomoStorage, DefaultSerdeBackend, KeccakHasher>,
    >(&temp_dir);
    let (block, _stake_amount, _eth_addresses) = new_simple_block();
    b.iter(|| block_on(env.run(block.clone(), || DummyPutter {})))
}

#[bench]
fn bench_application_commit_changes_rocksdb_jmt_keccak256_medium(b: &mut Bencher) {
    let temp_dir = tempdir().unwrap();
    let mut env = create_rocksdb_env::<
        JmtMerklizeProvider<AtomoStorage, DefaultSerdeBackend, KeccakHasher>,
    >(&temp_dir);
    let (block, _stake_amount, _eth_addresses) = new_medium_block();
    b.iter(|| block_on(env.run(block.clone(), || DummyPutter {})))
}

#[bench]
fn bench_application_commit_changes_rocksdb_jmt_keccak256_complex(b: &mut Bencher) {
    let temp_dir = tempdir().unwrap();
    let mut env = create_rocksdb_env::<
        JmtMerklizeProvider<AtomoStorage, DefaultSerdeBackend, KeccakHasher>,
    >(&temp_dir);
    let (block, _stake_amount, _eth_addresses, _node_public_keys) = new_complex_block();
    b.iter(|| block_on(env.run(block.clone(), || DummyPutter {})))
}

#[bench]
fn bench_application_commit_changes_rocksdb_jmt_sha256_simple(b: &mut Bencher) {
    let temp_dir = tempdir().unwrap();
    let mut env = create_rocksdb_env::<
        JmtMerklizeProvider<AtomoStorage, DefaultSerdeBackend, Sha256Hasher>,
    >(&temp_dir);
    let (block, _stake_amount, _eth_addresses) = new_simple_block();
    b.iter(|| block_on(env.run(block.clone(), || DummyPutter {})))
}

#[bench]
fn bench_application_commit_changes_rocksdb_jmt_sha256_medium(b: &mut Bencher) {
    let temp_dir = tempdir().unwrap();
    let mut env = create_rocksdb_env::<
        JmtMerklizeProvider<AtomoStorage, DefaultSerdeBackend, Sha256Hasher>,
    >(&temp_dir);
    let (block, _stake_amount, _eth_addresses) = new_medium_block();
    b.iter(|| block_on(env.run(block.clone(), || DummyPutter {})))
}

#[bench]
fn bench_application_commit_changes_rocksdb_jmt_sha256_complex(b: &mut Bencher) {
    let temp_dir = tempdir().unwrap();
    let mut env = create_rocksdb_env::<
        JmtMerklizeProvider<AtomoStorage, DefaultSerdeBackend, Sha256Hasher>,
    >(&temp_dir);
    let (block, _stake_amount, _eth_addresses, _node_public_keys) = new_complex_block();
    b.iter(|| block_on(env.run(block.clone(), || DummyPutter {})))
}

// MPT

#[bench]
fn bench_application_commit_changes_rocksdb_mpt_blake3_simple(b: &mut Bencher) {
    let temp_dir = tempdir().unwrap();
    let mut env = create_rocksdb_env::<
        MptMerklizeProvider<AtomoStorage, DefaultSerdeBackend, Blake3Hasher>,
    >(&temp_dir);
    let (block, _stake_amount, _eth_addresses) = new_simple_block();
    b.iter(|| block_on(env.run(block.clone(), || DummyPutter {})))
}

#[bench]
fn bench_application_commit_changes_rocksdb_mpt_blake3_medium(b: &mut Bencher) {
    let temp_dir = tempdir().unwrap();
    let mut env = create_rocksdb_env::<
        MptMerklizeProvider<AtomoStorage, DefaultSerdeBackend, Blake3Hasher>,
    >(&temp_dir);
    let (block, _stake_amount, _eth_addresses) = new_medium_block();
    b.iter(|| block_on(env.run(block.clone(), || DummyPutter {})))
}

#[bench]
fn bench_application_commit_changes_rocksdb_mpt_blake3_complex(b: &mut Bencher) {
    let temp_dir = tempdir().unwrap();
    let mut env = create_rocksdb_env::<
        MptMerklizeProvider<AtomoStorage, DefaultSerdeBackend, Blake3Hasher>,
    >(&temp_dir);
    let (block, _stake_amount, _eth_addresses, _node_public_keys) = new_complex_block();
    b.iter(|| block_on(env.run(block.clone(), || DummyPutter {})))
}

#[bench]
fn bench_application_commit_changes_rocksdb_mpt_keccak256_simple(b: &mut Bencher) {
    let temp_dir = tempdir().unwrap();
    let mut env = create_rocksdb_env::<
        MptMerklizeProvider<AtomoStorage, DefaultSerdeBackend, KeccakHasher>,
    >(&temp_dir);
    let (block, _stake_amount, _eth_addresses) = new_simple_block();
    b.iter(|| block_on(env.run(block.clone(), || DummyPutter {})))
}

#[bench]
fn bench_application_commit_changes_rocksdb_mpt_keccak256_medium(b: &mut Bencher) {
    let temp_dir = tempdir().unwrap();
    let mut env = create_rocksdb_env::<
        MptMerklizeProvider<AtomoStorage, DefaultSerdeBackend, KeccakHasher>,
    >(&temp_dir);
    let (block, _stake_amount, _eth_addresses) = new_medium_block();
    b.iter(|| block_on(env.run(block.clone(), || DummyPutter {})))
}

#[bench]
fn bench_application_commit_changes_rocksdb_mpt_keccak256_complex(b: &mut Bencher) {
    let temp_dir = tempdir().unwrap();
    let mut env = create_rocksdb_env::<
        MptMerklizeProvider<AtomoStorage, DefaultSerdeBackend, KeccakHasher>,
    >(&temp_dir);
    let (block, _stake_amount, _eth_addresses, _node_public_keys) = new_complex_block();
    b.iter(|| block_on(env.run(block.clone(), || DummyPutter {})))
}

#[bench]
fn bench_application_commit_changes_rocksdb_mpt_sha256_simple(b: &mut Bencher) {
    let temp_dir = tempdir().unwrap();
    let mut env = create_rocksdb_env::<
        MptMerklizeProvider<AtomoStorage, DefaultSerdeBackend, Sha256Hasher>,
    >(&temp_dir);
    let (block, _stake_amount, _eth_addresses) = new_simple_block();
    b.iter(|| block_on(env.run(block.clone(), || DummyPutter {})))
}

#[bench]
fn bench_application_commit_changes_rocksdb_mpt_sha256_medium(b: &mut Bencher) {
    let temp_dir = tempdir().unwrap();
    let mut env = create_rocksdb_env::<
        MptMerklizeProvider<AtomoStorage, DefaultSerdeBackend, Sha256Hasher>,
    >(&temp_dir);
    let (block, _stake_amount, _eth_addresses) = new_medium_block();
    b.iter(|| block_on(env.run(block.clone(), || DummyPutter {})))
}

#[bench]
fn bench_application_commit_changes_rocksdb_mpt_sha256_complex(b: &mut Bencher) {
    let temp_dir = tempdir().unwrap();
    let mut env = create_rocksdb_env::<
        MptMerklizeProvider<AtomoStorage, DefaultSerdeBackend, Sha256Hasher>,
    >(&temp_dir);
    let (block, _stake_amount, _eth_addresses, _node_public_keys) = new_complex_block();
    b.iter(|| block_on(env.run(block.clone(), || DummyPutter {})))
}
