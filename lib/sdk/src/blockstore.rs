use std::path::PathBuf;

use arrayvec::ArrayString;

use crate::ipc::BLOCKSTORE;

/// Returns the root blockstore.
///
/// # Panics
///
/// If called from outside of a service execution.
pub fn blockstore_root() -> &'static PathBuf {
    unsafe { BLOCKSTORE.as_ref().expect("setup not completed") }
}

/// Returns the path to a blockstore item with the given hash.
pub fn get_internal_path(hash: &[u8; 32]) -> PathBuf {
    blockstore_root().join(format!("./internal/{}", to_hex(hash)))
}

/// Returns the path to a blockstore block with the given block counter and hash.
pub fn get_block_path(counter: usize, block_hash: &[u8; 32]) -> PathBuf {
    blockstore_root().join(format!("./block/{counter}-{}", to_hex(block_hash)))
}

#[inline]
fn to_hex(slice: &[u8; 32]) -> ArrayString<64> {
    let mut s = ArrayString::new();
    let table = b"0123456789abcdef";
    for &b in slice {
        s.push(table[(b >> 4) as usize] as char);
        s.push(table[(b & 0xf) as usize] as char);
    }
    s
}
