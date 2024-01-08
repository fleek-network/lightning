use fleek_blake3::platform::{self, Platform};

use super::constants::*;

/// Returns the blake3 initialization vector implemented by [`fleek_blake3`].
#[inline(always)]
pub fn iv() -> fleek_blake3::tree::IV {
    fleek_blake3::tree::IV::new_keyed(&KEY)
}

#[inline(always)]
pub fn merge(
    platform: Platform,
    left_cv: &[u8; 32],
    right_cv: &[u8; 32],
    is_root: bool,
) -> [u8; 32] {
    let mut block = [0; 64];
    block[..32].copy_from_slice(left_cv);
    block[32..].copy_from_slice(right_cv);
    let mut cv = platform::words_from_le_bytes_32(&KEY);
    platform.compress_in_place(
        &mut cv,
        &block,
        64,
        0, // counter is always zero for parents.
        if is_root {
            B3_PARENT | B3_KEYED_HASH | B3_ROOT
        } else {
            B3_PARENT | B3_KEYED_HASH
        },
    );
    platform::le_bytes_from_words_32(&cv)
}
