use std::ops::Deref;

use super::b3::{platform, ROOT};
use super::{b3, dir_hasher, join};
use crate::hasher::b3::{CHUNK_END, CHUNK_START};
use crate::utils;

/// A small IV which fits into one memory word for when we have a static reference to the [IV].
struct SmallIV {
    iv: *const IV,
}

impl SmallIV {
    pub const DEFAULT: Self = Self::from_static_ref(&IV::DEFAULT);
    pub const DIR: Self = Self::from_static_ref(&IV::DIR);

    pub const fn from_static_ref(iv: &'static IV) -> Self {
        debug_assert!(iv.flags_internal & 1 == 0);
        Self {
            // Safety: The `iv` param has a static lifetime which means the pointer will be valid
            // for the rest of the duration of the program. And also given the debug assertion above
            // and the program invariant we have the last bit of the flag is set to 0. Which means
            // the Drop implementation will skip over trying to deallocate the memory.
            iv: unsafe { iv as *const IV },
        }
    }

    pub fn new(mut iv: IV) -> Self {
        debug_assert!(iv.flags_internal & 1 == 0);
        // Set the last bit to 1 so that Drop knows it has to dealloc the memory.
        iv.flags_internal |= 1;
        Self {
            iv: Box::into_raw(Box::new(iv)),
        }
    }
}

impl Deref for SmallIV {
    type Target = IV;
    fn deref(&self) -> &Self::Target {
        unsafe { &*self.iv }
    }
}

impl Drop for SmallIV {
    fn drop(&mut self) {
        if self.flags_internal & 1 == 1 {
            // Safety: Program invariant if the last bit is set to one we have gotten this
            // IV using [SmallIV::new] and not via an static reference.
            unsafe { Box::from_raw(self.iv as *mut IV) };
        }
    }
}

/// Initialization vector for a Blake3 hasher, if `BlockHasher::new()` or `HashTreeBuilder::new()`
/// are not sufficient for you and you need more customization on the IV, such as keyed hash
/// function, you can achieve it by constructing the [`IV`] first using a method like
/// [`IV::new_keyed`] and convert it to either of these by simply using the [`Into`] trait.
#[derive(Clone, Copy)]
pub struct IV {
    pub(crate) key: b3::CVWords,
    /// The first bit is never meant to be used for blake3 itself and we always clear it out
    /// when it is read using the [IV::flags] method.
    flags_internal: u8,
    empty: [u8; 32],
}

impl IV {
    pub const DEFAULT: Self = IV::new_internal(b3::IV, 0);
    pub const DIR: Self = IV::new_keyed(&dir_hasher::KEY);

    const fn new_internal(key: &b3::CVWords, flags: u8) -> Self {
        debug_assert!(flags & 1 == 0);

        Self {
            key: *key,
            flags_internal: flags,
            empty: empty_hash(key, flags),
        }
    }

    /// Returns the flag for this IV.
    pub const fn flags(&self) -> u8 {
        self.flags_internal & (u8::MAX - 1)
    }

    /// The keyed hash function.
    ///
    /// This is suitable for use as a message authentication code, for example to
    /// replace an HMAC instance. In that use case, the constant-time equality
    /// checking provided by [`Hash`](crate::Hash) is almost always a security
    /// requirement, and callers need to be careful not to compare MACs as raw
    /// bytes.
    pub const fn new_keyed(key: &[u8; b3::KEY_LEN]) -> Self {
        let key_words = utils::words_from_le_bytes_32(key);
        Self::new_internal(&key_words, b3::KEYED_HASH)
    }

    /// The key derivation function.
    ///
    /// Given cryptographic key material of any length and a context string of any
    /// length, this function outputs a 32-byte derived subkey. **The context string
    /// should be hardcoded, globally unique, and application-specific.** A good
    /// default format for such strings is `"[application] [commit timestamp]
    /// [purpose]"`, e.g., `"example.com 2019-12-25 16:18:03 session tokens v1"`.
    ///
    /// Key derivation is important when you want to use the same key in multiple
    /// algorithms or use cases. Using the same key with different cryptographic
    /// algorithms is generally forbidden, and deriving a separate subkey for each
    /// use case protects you from bad interactions. Derived keys also mitigate the
    /// damage from one part of your application accidentally leaking its key.
    pub fn new_derive_key(context: &str) -> Self {
        let context_key = b3::hash_all_at_once::<join::SerialJoin>(
            context.as_bytes(),
            b3::IV,
            b3::DERIVE_KEY_CONTEXT,
        )
        .root_hash();
        let context_key_words = platform::words_from_le_bytes_32(&context_key);
        Self::new_internal(&context_key_words, b3::DERIVE_KEY_MATERIAL)
    }
}

impl IV {
    /// Returns the hash of an empty input.
    pub const fn empty_hash(&self) -> &[u8; 32] {
        &self.empty
    }

    /// Merge two children into the parent hash, the `is_root` determines if the result
    /// is the finalized hash.
    pub fn merge(&self, left: &[u8; 32], right: &[u8; 32], is_root: bool) -> [u8; 32] {
        let output = b3::parent_node_output(
            left,
            right,
            &self.key,
            self.flags(),
            platform::Platform::detect(),
        );
        if is_root {
            output.root_hash()
        } else {
            output.chaining_value()
        }
    }

    /// Returns the hash of all of the input data at once.
    pub fn hash_all_at_once(&self, input: &[u8]) -> [u8; 32] {
        b3::hash_all_at_once::<join::SerialJoin>(input, &self.key, self.flags()).root_hash()
    }

    /// Returns the hash of all of the input data at once using rayon for parallelization.
    pub fn hash_all_at_once_rayon(&self, input: &[u8]) -> [u8; 32] {
        b3::hash_all_at_once::<join::RayonJoin>(input, &self.key, self.flags()).root_hash()
    }
}

impl Default for IV {
    fn default() -> Self {
        Self::DEFAULT
    }
}

/// Return the empty hash under the given key and flags. This is a const-fn which allows us to use
/// it in other const-fn constructors.
const fn empty_hash(key: &b3::CVWords, flags: u8) -> [u8; 32] {
    let flags = flags | CHUNK_START | CHUNK_END | ROOT;
    let mut state = [
        key[0],
        key[1],
        key[2],
        key[3],
        key[4],
        key[5],
        key[6],
        key[7],
        b3::IV[0],
        b3::IV[1],
        b3::IV[2],
        b3::IV[3],
        0, // counter_low
        0, // counter_high
        0, // block_len
        flags as u32,
    ];

    state = round(state);
    state = round(state);
    state = round(state);
    state = round(state);
    state = round(state);
    state = round(state);
    state = round(state);

    let mut cv = [0u32; 8];
    cv[0] = state[0] ^ state[8];
    cv[1] = state[1] ^ state[9];
    cv[2] = state[2] ^ state[10];
    cv[3] = state[3] ^ state[11];
    cv[4] = state[4] ^ state[12];
    cv[5] = state[5] ^ state[13];
    cv[6] = state[6] ^ state[14];
    cv[7] = state[7] ^ state[15];
    utils::le_bytes_from_words_32(&cv)
}

#[inline(always)]
const fn round(mut state: [u32; 16]) -> [u32; 16] {
    // Mix the columns.
    state = g(state, 0, 4, 8, 12);
    state = g(state, 1, 5, 9, 13);
    state = g(state, 2, 6, 10, 14);
    state = g(state, 3, 7, 11, 15);
    // Mix the diagonals.
    state = g(state, 0, 5, 10, 15);
    state = g(state, 1, 6, 11, 12);
    state = g(state, 2, 7, 8, 13);
    g(state, 3, 4, 9, 14)
}

#[inline(always)]
const fn g(mut state: [u32; 16], a: usize, b: usize, c: usize, d: usize) -> [u32; 16] {
    state[a] = state[a].wrapping_add(state[b]);
    state[d] = (state[d] ^ state[a]).rotate_right(16);
    state[c] = state[c].wrapping_add(state[d]);
    state[b] = (state[b] ^ state[c]).rotate_right(12);
    state[a] = state[a].wrapping_add(state[b]);
    state[d] = (state[d] ^ state[a]).rotate_right(8);
    state[c] = state[c].wrapping_add(state[d]);
    state[b] = (state[b] ^ state[c]).rotate_right(7);
    state
}

#[cfg(test)]
mod tests {
    use rand::{thread_rng, Rng};

    use super::*;

    #[test]
    fn empty_hash() {
        assert_eq!(*IV::DIR.empty_hash(), dir_hasher::EMPTY_HASH);
        assert_eq!(*IV::DIR.empty_hash(), IV::DIR.hash_all_at_once(&[]));
        assert_eq!(
            *IV::default().empty_hash(),
            IV::default().hash_all_at_once(&[])
        );
        let mut rng = thread_rng();
        for _ in 0..32 {
            let key = rng.gen();
            let iv = IV::new_keyed(&key);
            assert_eq!(*iv.empty_hash(), iv.hash_all_at_once(&[]));
        }
    }
}
