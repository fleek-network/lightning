#![no_main]

use arbitrary::Arbitrary;
use blake3_tree::*;
use libfuzzer_sys::fuzz_target;

#[derive(Arbitrary, Debug)]
struct FuzzInput {
    size: u16,
}

#[inline(always)]
fn block_data(n: usize) -> [u8; 256 * 1024] {
    let mut data = [0; 256 * 1024];
    for i in data.chunks_exact_mut(2) {
        i[0] = n as u8;
        i[1] = (n / 256) as u8;
    }
    data
}

fuzz_target!(|data: FuzzInput| {
    let size = (data.size & ((1 << 12) - 1)) as usize + 1;
    let start = 0;

    let mut tree_builder = blake3::tree::HashTreeBuilder::new();
    (0..size).for_each(|i| tree_builder.update(&block_data(i)));
    let output = tree_builder.finalize();

    let mut verifier = IncrementalVerifier::new(*output.hash.as_bytes(), 0);
    verifier.preserve_tree();

    verifier
        .feed_proof(ProofBuf::new(&output.tree, 0).as_slice())
        .expect(&format!("Invalid Proof: size={size}"));

    verifier
        .verify({
            let mut block = blake3::tree::BlockHasher::new();
            block.set_block(start);
            block.update(&block_data(start));
            block
        })
        .expect(&format!("Invalid Content: size={size}"));

    for i in start + 1..size {
        let target_index = i * 2 - i.count_ones() as usize;

        verifier
            .feed_proof(ProofBuf::resume(&output.tree, i).as_slice())
            .expect(&format!("Invalid Proof on Resume: size={size} i={i}"));

        verifier
            .verify_hash(&output.tree[target_index])
            .expect(&format!("Invalid Content on Resume: size={size} i={i}"));
    }

    assert_eq!(
        verifier.is_done(),
        true,
        "verifier not terminated: size={size}"
    );

    if verifier.take_tree() != output.tree {
        panic!("Captured tree does not match the initial tree.");
    }
});
