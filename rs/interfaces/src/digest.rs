/// Any object that implements the cryptographic digest function, this should
/// use a collision resistant hash function and have a representation agnostic
/// hashing for our core objects.
pub trait ToDigest {
    /// Returns the digest of the object.
    fn to_digest(&self) -> [u8; 32];
}
