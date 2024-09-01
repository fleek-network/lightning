use fdi::BuildGraph;
use lightning_schema::broadcast::ResolvedImmutablePointerRecord;

use crate::components::NodeComponents;
use crate::types::{Blake3Hash, ImmutablePointer};

/// The resolver is responsible to resolve an FNIP (Fleek Network Immutable Pointer),
/// into a Blake3 hash of the content.
#[interfaces_proc::blank]
pub trait ResolverInterface<C: NodeComponents>: BuildGraph + Sized + Send + Sync + Clone {
    type OriginFinder: OriginFinderAsyncIter;

    /// Publish new records into the resolver global hash table about us witnessing
    /// the given blake3 hash from resolving the following pointers.
    async fn publish(&self, hash: Blake3Hash, pointers: &[ImmutablePointer]);

    /// Tries to find the blake3 hash of an immutable pointer by only relying on locally cached
    /// records and without performing any contact with other nodes.
    ///
    /// This can return [`None`] if no local record is found.
    async fn get_blake3_hash(&self, pointer: ImmutablePointer) -> Option<Blake3Hash>;

    /// Returns an origin finder that can yield origins for the provided blake3 hash.
    fn get_origin_finder(&self, hash: Blake3Hash) -> Self::OriginFinder;

    /// Returns all origins in the local db
    fn get_origins(&self, hash: Blake3Hash) -> Option<Vec<ResolvedImmutablePointerRecord>>;
}

/// An `async-iterator`-like interface that tries to find the immutable pointers of
#[interfaces_proc::blank]
pub trait OriginFinderAsyncIter: Sized + Send + Sync {
    /// Returns the hash of requested content.
    fn hash(&self) -> &Blake3Hash;

    /// Find and return the next origin for the requested hash. Returns `None`
    /// after the implementation defined timeout has passed.
    async fn next(&mut self) -> Option<ImmutablePointer>;

    /// The sync version of `next`. This returns `None` if there are no further
    /// items in our outgoing queue. So this function can return `None` while
    /// the subsequent call to `next` can return `Some`.
    ///
    /// This is only a way to access the internal state of the iterator when several
    /// items are already found.
    fn next_sync(&mut self) -> Option<ImmutablePointer>;
}
