use affair::Socket;
use anyhow;

use crate::{
    infu_collection::Collection, ConfigConsumer, ConfigProviderInterface, WithStartAndShutdown,
};

/// A socket for submitting a fetch request to an origin.
pub type OriginProviderSocket<Stream> = Socket<Vec<u8>, anyhow::Result<Stream>>;

/// The abstraction layer for different origins and how we handle them in the codebase in
/// a modular way, and [`OriginProvider`] can be something like a provider for resolving
/// *IPFS* files.
#[infusion::service]
pub trait OriginProviderInterface<C: Collection>:
    ConfigConsumer + WithStartAndShutdown + Sized + Send + Sync
{
    fn _init(config: ::ConfigProviderInterface) {
        Self::init(config.get::<Self>())
    }

    type Stream: UntrustedStream = BlankUntrustedStream;

    /// Initialize the origin service.
    fn init(config: Self::Config) -> anyhow::Result<Self>;

    /// Returns a socket for submitting a fetch request to an origin.
    fn get_socket(&self) -> OriginProviderSocket<Self::Stream>;
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

pub struct BlankUntrustedStream;

impl tokio_stream::Stream for BlankUntrustedStream {
    type Item = Result<bytes::Bytes, std::io::Error>;

    fn poll_next(
        self: std::pin::Pin<&mut Self>,
        _cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<Self::Item>> {
        todo!()
    }
}

impl UntrustedStream for BlankUntrustedStream {
    fn was_content_valid(&self) -> Option<bool> {
        todo!()
    }
}
