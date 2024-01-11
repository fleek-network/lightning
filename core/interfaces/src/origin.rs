use affair::Socket;
use anyhow;

use crate::infu_collection::Collection;
use crate::types::Blake3Hash;
use crate::{BlockStoreInterface, ConfigConsumer, ConfigProviderInterface, WithStartAndShutdown};

/// A socket for submitting a fetch request to an origin.
pub type OriginProviderSocket = Socket<Vec<u8>, anyhow::Result<Blake3Hash>>;

/// The abstraction layer for different origins and how we handle them in the codebase in
/// a modular way, and [`OriginProvider`] can be something like a provider for resolving
/// *IPFS* files.
#[infusion::service]
pub trait OriginProviderInterface<C: Collection>:
    ConfigConsumer + WithStartAndShutdown + Sized + Send + Sync
{
    fn _init(config: ::ConfigProviderInterface, blockstore: ::BlockStoreInterface) {
        Self::init(config.get::<Self>(), blockstore.clone())
    }

    /// Initialize the origin service.
    fn init(config: Self::Config, blockstore: C::BlockStoreInterface) -> anyhow::Result<Self>;

    /// Returns a socket for submitting a fetch request to an origin.
    fn get_socket(&self) -> OriginProviderSocket;
}

#[infusion::service]
pub trait OriginFetcherInterface<C: Collection>:
    ConfigConsumer + Clone + Sized + Send + Sync
{
    fn _init(config: ::ConfigProviderInterface, blockstore: ::BlockStoreInterface) {
        Self::init(config.get::<Self>(), blockstore.clone())
    }

    fn init(config: Self::Config, blockstore: C::BlockStoreInterface) -> anyhow::Result<Self>;

    async fn fetch(&self, identifier: Vec<u8>) -> anyhow::Result<Blake3Hash>;
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
