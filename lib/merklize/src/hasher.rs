use anyhow::Result;
use schemars::JsonSchema;

/// A trait for a simple hasher that can hash data and return a 32-byte array.
pub trait SimpleHasher: Sized + Clone + Send + Sync {
    const ICS23_HASH_OP: ics23::HashOp;

    fn new() -> Self;

    fn update(&mut self, data: &[u8]);

    fn finalize(self) -> [u8; 32];

    fn hash(data: impl AsRef<[u8]>) -> [u8; 32] {
        let mut hasher = Self::new();
        hasher.update(data.as_ref());
        hasher.finalize()
    }
}

/// A simple hash type that wraps a 32-byte array.
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, JsonSchema)]
pub struct SimpleHash([u8; 32]);

impl SimpleHash {
    pub fn to_vec(&self) -> Vec<u8> {
        self.0.to_vec()
    }
}

impl serde::Serialize for SimpleHash {
    /// Serialize the hash as a hex string.
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        hex::serde::serialize(self.0, serializer)
    }
}

impl<'de> serde::Deserialize<'de> for SimpleHash {
    /// Deserialize the hash from a hex string.
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        hex::serde::deserialize(deserializer).map(SimpleHash)
    }
}

impl Default for SimpleHash {
    /// Create a new `SimpleHash` with all zeros.
    fn default() -> Self {
        Self([0; 32])
    }
}

impl SimpleHash {
    pub fn build<H: SimpleHasher>(key: impl AsRef<[u8]>) -> Self {
        Self(H::hash(key.as_ref()))
    }

    pub fn from_hex(hex: &str) -> Result<Self> {
        Ok(hex::decode(hex)?.into())
    }
}

impl From<[u8; 32]> for SimpleHash {
    /// Create a new `SimpleHash` from a 32-byte array.
    fn from(hash: [u8; 32]) -> Self {
        Self(hash)
    }
}

impl From<SimpleHash> for [u8; 32] {
    /// Convert a `SimpleHash` to a 32-byte array.
    fn from(hash: SimpleHash) -> Self {
        hash.0
    }
}

impl From<SimpleHash> for Vec<u8> {
    fn from(hash: SimpleHash) -> Self {
        hash.0.to_vec()
    }
}

impl From<Vec<u8>> for SimpleHash {
    fn from(hash: Vec<u8>) -> Self {
        Self(hash.try_into().expect("Incorrect length for hash"))
    }
}

impl AsRef<[u8]> for SimpleHash {
    /// Get a reference to the hash byte array.
    fn as_ref(&self) -> &[u8] {
        &self.0
    }
}

impl PartialEq<&str> for SimpleHash {
    fn eq(&self, other: &&str) -> bool {
        &self.to_string() == other
    }
}

impl PartialEq<SimpleHash> for &str {
    fn eq(&self, other: &SimpleHash) -> bool {
        self == &other.to_string()
    }
}

impl core::fmt::Display for SimpleHash {
    /// Display the hash as a hex string.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(serde_json::to_string(&self).unwrap().trim_matches('"'))
    }
}

impl core::fmt::Debug for SimpleHash {
    /// Represent the hash as a hex JSON string in debug output.
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(serde_json::to_string(&self).unwrap().as_str())
    }
}
