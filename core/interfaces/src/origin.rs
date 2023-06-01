/// The abstraction layer for different origins and how we handle them in the codebase in
/// a modular way, and [`OriginProvider`] can be something like a provider for resolving
/// *IPFS* files.
pub trait OriginProviderInterface<Stream: tokio_stream::Stream<Item = bytes::BytesMut>>:
    Send + Sync
{
    /// Fetch the content from the provided `uri`.
    fn fetch(&self, uri: &[u8]) -> Stream;
}
