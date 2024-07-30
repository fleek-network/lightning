//! Constants used across different parts of the algorithm.

/// Blake3 hash of the word `"DIRECTORY"` used as the key for the hashing.
pub const KEY: [u8; 32] = [
    139, 88, 112, 131, 96, 138, 152, 197, 238, 63, 142, 210, 224, 88, 97, 183, 244, 210, 116, 213,
    84, 215, 9, 16, 21, 175, 61, 72, 251, 174, 76, 21,
];

/// Hash of an empty directory which is set to `KeyedHash(KEY, &[])`.
pub const EMPTY_HASH: [u8; 32] = [
    72, 63, 122, 133, 174, 60, 219, 10, 52, 209, 178, 47, 200, 109, 164, 116, 12, 53, 178, 104,
    128, 89, 147, 234, 130, 71, 29, 80, 131, 193, 231, 128,
];

// Flags used for writing a directory entry transcript.

pub const IS_ROOT_FLAG: u8 = 1;
pub const IS_FILE_FLAG: u8 = 2;
pub const IS_DIR_FLAG: u8 = 4;
pub const IS_SYM_FLAG: u8 = 8;

// Flags borrowed from blake3 spec used in the implementation of the merge.

pub const B3_ROOT: u8 = 1 << 3;
pub const B3_PARENT: u8 = 1 << 2;
pub const B3_KEYED_HASH: u8 = 1 << 4;
