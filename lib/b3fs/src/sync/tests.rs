use std::env::temp_dir;

use crate::hasher::byte_hasher::BlockHasher;
use crate::hasher::collector::BufCollector;
use crate::stream::verifier::{IncrementalVerifier, WithHashTreeCollector};
use crate::sync::bucket::file::writer::FileWriter;
use crate::sync::bucket::tests::get_random_file;
use crate::sync::bucket::Bucket;

#[test]
fn test_read_from_bucket_and_verify() {
    // Put file into bucket
    let n_blocks = 10;
    let mut temp_dir = temp_dir();
    temp_dir.push("test-read-verify");
    let bucket = Bucket::open(&temp_dir).unwrap();
    let mut writer = FileWriter::new(&bucket).unwrap();
    let data = get_random_file(8192 * n_blocks);

    writer.write(&data).unwrap();
    let root_hash = writer.commit().unwrap();

    // Read from the bucket and verify
    let load_content = bucket.get(&root_hash).unwrap();
    let num_blocks = load_content.blocks();
    let file_read = load_content.into_file().unwrap();

    let mut tree = file_read.hashtree().unwrap();
    let mut verifier = IncrementalVerifier::<WithHashTreeCollector<BufCollector>>::default();
    verifier.set_root_hash(root_hash);

    let mut recv_data = Vec::new();
    for block in 0..num_blocks {
        let mut hasher = BlockHasher::default();
        hasher.set_block(block as usize);

        let hash = tree.get_hash(block).unwrap().unwrap();
        let chunk = bucket.get_block_content(&hash).unwrap().unwrap();

        hasher.update(&chunk);
        let exp_hash = hasher.finalize(false);

        // zero block number will give empty proof
        // but it needs to be fed anyways
        let proof = tree.generate_proof(block).unwrap();
        verifier
            .feed_proof(proof.as_slice())
            .expect("verification failed");
        verifier.verify_hash(exp_hash).unwrap();

        recv_data.extend(chunk);
    }

    // Do a final sanity check (only for testing)
    let check_hash = fleek_blake3::hash(&recv_data);
    assert_eq!(root_hash, *check_hash.as_bytes());
}
