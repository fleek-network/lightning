use lightning_interfaces::types::{Blake3Hash, ImmutablePointer};
use lightning_interfaces::OriginFinderAsyncIter;

pub struct OriginFinder {}

impl OriginFinderAsyncIter for OriginFinder {
    /// Returns the hash of requested content.
    fn hash(&self) -> &Blake3Hash {
        todo!()
    }

    /// Find and return the next origin for the requested hash. Returns `None`
    /// after the implementation defined timeout has passed.
    async fn next(&mut self) -> Option<ImmutablePointer> {
        todo!()
    }

    /// The sync version of `next`. This returns `None` if there are no further
    /// items in our outgoing queue. So this function can return `None` while
    /// the subsequent call to `next` can return `Some`.
    ///
    /// This is only a way to access the internal state of the iterator when several
    /// items are already found.
    fn next_sync(&mut self) -> Option<ImmutablePointer> {
        todo!()
    }
}
