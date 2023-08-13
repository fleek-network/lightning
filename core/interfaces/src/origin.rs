use affair::Socket;
use anyhow;
use async_trait::async_trait;

use crate::{ConfigConsumer, WithStartAndShutdown};

/// A socket for submitting a fetch request to an origin.
pub type OriginProviderSocket<Stream> = Socket<Vec<u8>, anyhow::Result<Stream>>;

/// The abstraction layer for different origins and how we handle them in the codebase in
/// a modular way, and [`OriginProvider`] can be something like a provider for resolving
/// *IPFS* files.
#[async_trait]
pub trait OriginProviderInterface<Stream: UntrustedStream>:
    ConfigConsumer + WithStartAndShutdown + Sized + Send + Sync
{
    /// Initialize the origin service.
    fn init(config: Self::Config) -> anyhow::Result<Self>;

    /// Returns a socket for submitting a fetch request to an origin.
    fn get_socket(&self) -> OriginProviderSocket<Stream>;
}

/// An untrusted stream to an origin, this allows the origin provider to start the
/// streaming of the content it receives before it is sure of the integrity of the
/// content.
///
/// This way we will not have to store the entire data in memory for the sake of
/// verification.
pub trait UntrustedStream:
    tokio_stream::Stream<Item = Result<bytes::Bytes, std::io::Error>>
{
    /// Returns true if the stream was valid. This is meant to be called at the end
    /// of the stream. If this method is called before the end of the stream, it will return
    /// `None`.
    fn was_content_valid(&self) -> Option<bool>;
}
