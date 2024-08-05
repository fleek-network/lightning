#[cfg(feature = "hasher-blake3")]
pub mod blake3;

#[cfg(feature = "hasher-keccak")]
pub mod keccak;

#[cfg(feature = "hasher-sha2")]
pub mod sha2;
