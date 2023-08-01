/// The abstraction layer for different origins and how we handle them in the codebase in
/// a modular way, and [`OriginProvider`] can be something like a provider for resolving
/// *IPFS* files.
pub trait OriginProviderInterface<Stream: UntrustedStream>: Send + Sync {
    /// Fetch the content from the provided `uri`. A uri is anything the origin
    /// accepts.
    fn fetch(&self, uri: &[u8]) -> Stream;
}

/// An untrusted stream to an origin, this allows the origin provider to start the
/// streaming of the content it receives before it is sure of the integrity of the
/// content.
///
/// This way we will not have to store the entire data in memory for the sake of
/// verification.
pub trait UntrustedStream: tokio_stream::Stream<Item = bytes::BytesMut> {
    /// Returns true if the stream was valid. This is meant to be called at the end
    /// of the stream.
    fn was_content_valid(&self) -> bool;
}
