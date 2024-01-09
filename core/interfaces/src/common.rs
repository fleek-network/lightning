use std::future::Future;

pub trait WithStartAndShutdown {
    /// Returns true if this system is running or not.
    fn is_running(&self) -> bool;

    /// Start the system, should not do anything if the system is already
    /// started.
    fn start(&self) -> impl Future<Output = ()> + Send;

    /// Send the shutdown signal to the system.
    fn shutdown(&self) -> impl Future<Output = ()> + Send;
}

/// Any object that implements the cryptographic digest function, this should
/// use a collision resistant hash function and have a representation agnostic
/// hashing for our core objects. Re-exported from [`ink_quill`]
pub use ink_quill::ToDigest;
pub use ink_quill::TranscriptBuilder;

impl<T> WithStartAndShutdown for infusion::Blank<T> {
    /// Returns true if this system is running or not.
    fn is_running(&self) -> bool {
        true
    }

    /// Start the system, should not do anything if the system is already
    /// started.
    async fn start(&self) {}

    /// Send the shutdown signal to the system.
    async fn shutdown(&self) {}
}
