use async_trait::async_trait;

#[async_trait]
pub trait WithStartAndShutdown {
    /// Returns true if this system is running or not.
    fn is_running(&self) -> bool;

    /// Start the system, should not do anything if the system is already
    /// started.
    async fn start(&self);

    /// Send the shutdown signal to the system.
    async fn shutdown(&self);
}

/// Any object that implements the cryptographic digest function, this should
/// use a collision resistant hash function and have a representation agnostic
/// hashing for our core objects.
pub trait ToDigest {
    /// Returns the digest of the object.
    fn to_digest(&self) -> [u8; 32];
}
