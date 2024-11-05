use std::fmt::Debug;
use std::path::{Path, PathBuf};

use arrayvec::ArrayString;
use rand::random;

/// Convert a hash digest to a human-readable string.
#[inline]
pub fn to_hex(slice: &[u8; 32]) -> ArrayString<64> {
    let mut s = ArrayString::new();
    let table = b"0123456789abcdef";
    for &b in slice {
        s.push(table[(b >> 4) as usize] as char);
        s.push(table[(b & 0xf) as usize] as char);
    }
    s
}

pub fn from_hex(hex: &ArrayString<64>) -> [u8; 32] {
    let mut out = [0; 32];
    let mut i = 0;
    let mut j = 0;
    while i < 64 {
        let byte = u8::from_str_radix(&hex[i..i + 2], 16).unwrap();
        out[j] = byte;
        j += 1;
        i += 2;
    }
    out
}

/// Returns the previous power of two of a given number, the returned
/// value is always less than the provided `n`.
#[inline(always)]
pub const fn previous_pow_of_two(n: usize) -> usize {
    n.next_power_of_two() / 2
}

/// The largest power of two less than or equal to `n`,.
#[inline]
pub fn largest_power_of_two_leq(n: usize) -> usize {
    ((n / 2) + 1).next_power_of_two()
}

/// Compute the index of the n-th leaf in the array representation of the tree.
/// see: <https://oeis.org/A005187>
#[inline(always)]
pub const fn tree_index(block_counter: usize) -> usize {
    2 * block_counter - block_counter.count_ones() as usize
}

/// Inverse of [`tree_index`].
pub const fn block_counter_from_tree_index(index: usize) -> Option<usize> {
    let mut result = 0;
    let mut x = index;
    // tree_index(7)=11 ; 7 = 4+2+1
    // 11 = 4 + (2*4-1)
    // 4  = 1 + (2*2-1)
    // 1  = 0 + (2*1-1)
    //
    // f(11) = f(4) + 4
    // f(4) = f(1) + 2
    // f(1) = f(0) + 1
    // f(0) = 0
    //
    // f(n) = f(r) + 2^e(n) such that: n = r + (2*2^e(n) - 1) and 0<=r<=2^e(n)
    // solving for e(n) we have e(n)=floor(log2((n + 1) / 2))=floor(log2(n + 1)) - 1
    while x != 0 {
        // compute the number of elements in the largest left subtree that it is
        // itself a full power of 2 elements.
        // =floor(log2(x + 1))
        let exp = usize::BITS - (x + 1).leading_zeros() - 1;
        let m = 1usize << (exp - 1); // 2^(exp - 1)
        // find our position in the right subtree and perform
        // the next step recursivly.
        x -= 2 * m - 1;
        // our index is at least as largest as all of the items in our left
        // subtree. All of the `m`s we are adding are unique power of twos
        // so we can use the `|=` instead which looks cooler.
        result |= m;
    }
    if tree_index(result) == index {
        Some(result)
    } else {
        None
    }
}

/// Given the number of hashes, returns the count of items in the hash tree.
#[inline(always)]
pub const fn hashtree_size_to_node_count(n: usize) -> usize {
    (n + 1) >> 1
}

/// Validates that the provided number of bytes is a valid number of bytes for a proof
/// buffer. This is only applicable to hashtree inclusion proofs.
#[inline(always)]
pub const fn is_valid_proof_len(n: usize) -> bool {
    const SEG_SIZE: usize = 32 * 8 + 1;
    // get the size of the first segment. we should either deal with a full segment (or n == 0)
    // or a valid partial segment. a valid partial segment has at least 1 item. so just a single
    // sign byte is invalid. then we keep the sign byte away `s - 1` and this should be a valid
    // set of full hashes and a multiple of 32. Also the proof must contain at least 2 hashes,
    // otherwise no merge operation can happen.
    let s = n % SEG_SIZE;
    s == 0 || (n > 64 && ((s - 1) % 32 == 0))
}

/// Returns true if `n` hashes can form a valid hash tree.
#[inline(always)]
pub const fn is_valid_tree_len(n: usize) -> bool {
    // for k entries, tree size = 2k - 1 = n
    // 2k = n + 1 -> (n + 1) % 2 == 0
    (n + 1) & 1 == 0
}

/// Returns true if the given number of bytes is valid for a file name. A file name on Unix has
/// a maximum of 255 bytes. And can not be empty.
#[inline(always)]
pub const fn is_valid_filename_len(n: usize) -> bool {
    n > 0 && n < 256
}

/// Returns true if the given number of bytes is valid for content of a symbolic link. A link
/// content may not execeed 1023 bytes.
#[inline(always)]
pub const fn is_valid_symlink_len(n: usize) -> bool {
    n > 0 && n < 1024
}

/// Check the given byte and returns true if it can be a valid file name. This checks the validity
/// of the length and ensures that a 'null' byte is not present in the bytes.
#[inline]
pub fn is_valid_filename(bytes: &[u8]) -> bool {
    is_valid_filename_len(bytes.len()) && !bytes.iter().any(|b| *b == 0)
}

/// Check the given byte and returns true if it can be a valid content for a symbolic link.
/// This checks the validity of the length and ensures that a 'null' byte is not present in
/// the bytes.
#[inline]
pub fn is_valid_symlink(bytes: &[u8]) -> bool {
    is_valid_symlink_len(bytes.len()) && !bytes.iter().any(|b| *b == 0)
}

/// Flatten a nested slice.
///
/// From standard library, but it's unstable there so here is a copy because it's good enough
/// for us...
#[inline(always)]
pub fn flatten<const N: usize, T>(slice: &[[T; N]]) -> &[T] {
    let len = if std::mem::size_of::<T>() == 0 {
        slice.len().checked_mul(N).expect("slice len overflow")
    } else {
        // SAFETY: `self.len() * N` cannot overflow because `self` is
        // already in the address space.
        slice.len() * N
    };
    // SAFETY: `[T]` is layout-identical to `[T; N]`
    unsafe { std::slice::from_raw_parts(slice.as_ptr().cast(), len) }
}

// const-fn implementation.
#[inline(always)]
pub const fn words_from_le_bytes_32(bytes: &[u8; 32]) -> [u32; 8] {
    #[inline(always)]
    const fn w(bytes: &[u8; 32], offset: usize) -> u32 {
        u32::from_le_bytes([
            bytes[offset],
            bytes[offset + 1],
            bytes[offset + 2],
            bytes[offset + 3],
        ])
    }
    let mut out = [0; 8];
    out[0] = w(bytes, 0);
    out[1] = w(bytes, 4);
    out[2] = w(bytes, 2 * 4);
    out[3] = w(bytes, 3 * 4);
    out[4] = w(bytes, 4 * 4);
    out[5] = w(bytes, 5 * 4);
    out[6] = w(bytes, 6 * 4);
    out[7] = w(bytes, 7 * 4);
    out
}

// const-fn implementation.
#[inline(always)]
pub const fn le_bytes_from_words_32(words: &[u32; 8]) -> [u8; 32] {
    let mut out = [0; 32];
    let mut i = 0;
    let mut j = 0;
    while i < 8 {
        let bytes = words[i].to_le_bytes();
        out[j] = bytes[0];
        j += 1;
        out[j] = bytes[1];
        j += 1;
        out[j] = bytes[2];
        j += 1;
        out[j] = bytes[3];
        j += 1;
        i += 1;
    }
    out
}

/// Used internally as a helper to pretty print hashes as hex strings.
pub struct Digest<'d>(pub &'d [u8; 32]);

impl<'d> Debug for Digest<'d> {
    #[inline(always)]
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", to_hex(self.0))
    }
}

/// Used internally as a helper to pretty print hashes as hex strings.
pub struct OwnedDigest(pub [u8; 32]);

impl Debug for OwnedDigest {
    #[inline(always)]
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", to_hex(&self.0))
    }
}

#[inline(always)]
pub fn random_file_from(path: &Path) -> PathBuf {
    let random_file: [u8; 32] = random();
    path.to_path_buf().join(to_hex(&random_file).as_str())
}

#[macro_export]
#[macro_use]
macro_rules! on_future {
    ($self:expr, $flag:expr, $new_state:expr, $on_ok:expr) => {
        match $flag {
            Poll::Ready(Ok(r)) => return $on_ok(r),
            Poll::Ready(Err(e)) => {
                if e.kind() == std::io::ErrorKind::UnexpectedEof {
                    $self.state = $new_state;
                    return Poll::Ready(None);
                } else {
                    return Poll::Ready(Some(Err(errors::ReadError::from(e))));
                }
            },
            Poll::Pending => return Poll::Pending,
        }
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_valid_proof_len() {
        assert!(is_valid_proof_len(0));
        assert!(!is_valid_proof_len(1));
        assert!(!is_valid_proof_len(32));
        assert!(!is_valid_proof_len(33));
        assert!(!is_valid_proof_len(64));
        assert!(is_valid_proof_len(65));
        assert!(!is_valid_proof_len(66));
        assert!(!is_valid_proof_len(33));
    }

    #[test]
    fn tree_index_inv() {
        for i in 0..100_000 {
            assert_eq!(Some(i), block_counter_from_tree_index(tree_index(i)));
        }
    }
}
