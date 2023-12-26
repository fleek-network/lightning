//! Javascript runtime bindings for the SDK APIs

use anyhow::anyhow;
use arrayref::array_ref;
use blake3_tree::utils::{tree_index, HashVec};
use deno_core::{extension, op2};
use fn_sdk::blockstore::get_internal_path;
use tracing::info;

#[op2(fast)]
pub fn log(#[string] message: String) {
    info!("Runtime: {message}");
}

#[op2(async)]
pub async fn fetch_blake3(#[buffer(copy)] hash: Vec<u8>) -> anyhow::Result<bool> {
    if hash.len() != 32 {
        return Err(anyhow!("blake3 hash must be 32 bytes"));
    }

    Ok(fn_sdk::api::fetch_blake3(*array_ref![hash, 0, 32]).await)
}

#[op2(async)]
#[buffer]
pub async fn load_content(#[buffer(copy)] hash: Vec<u8>) -> anyhow::Result<Box<[u8]>> {
    let path = get_internal_path(array_ref![hash, 0, 32]);

    // TODO: store proof on rust side, and only give javascript an id to reference the handle
    let proof = std::fs::read(path)?.into_boxed_slice();
    if proof.len() & 31 != 0 {
        return Err(anyhow!("corrupted proof in blockstore"));
    }

    Ok(proof)
}

#[op2(async)]
#[buffer]
pub async fn read_block(
    #[buffer(copy)] proof: Box<[u8]>,
    #[bigint] index: usize,
) -> anyhow::Result<Vec<u8>> {
    let tree = HashVec::from_inner(proof);
    let inner_hash = tree[tree_index(index)];
    let path = fn_sdk::blockstore::get_block_path(index, &inner_hash);
    let block = std::fs::read(path)?;

    Ok(block)
}

extension!(
    fleek,
    ops = [log, fetch_blake3, load_content, read_block],
    esm_entry_point = "ext:fleek/entrypoint.js",
    esm = [ dir "src/extensions", "entrypoint.js" ]
);
