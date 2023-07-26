use async_trait::async_trait;
use fleek_crypto::ClientPublicKey;
use tokio::io::{AsyncRead, AsyncWrite};

use crate::{common::WithStartAndShutdown, config::ConfigConsumer, CompressionAlgoSet};

#[async_trait]
pub trait HandshakeInterface: ConfigConsumer + WithStartAndShutdown + Sized + Send + Sync {
    type Connection: ConnectionInterface;

    /// Initialize a new delivery acknowledgment aggregator.
    async fn init(config: Self::Config) -> anyhow::Result<Self>;
}

pub trait ConnectionInterface: Send + Sync {
    /// The writer half of this connection.
    type Writer: AsyncWrite + Unpin + Send + Sync;

    /// The reader half of this connection.
    type Reader: AsyncRead + Unpin + Send + Sync;

    /// Split the connection to the `writer` and `reader` half and returns a mutable reference to
    /// both sides.
    fn split(&mut self) -> (&mut Self::Writer, &mut Self::Reader);

    /// Returns a mutable reference to the writer half of this connection.
    fn writer(&mut self) -> &mut Self::Writer;

    /// Returns a mutable reference to the reader half of this connection.
    fn reader(&mut self) -> &mut Self::Reader;

    /// Returns the lane number associated with this connection.
    fn get_lane(&self) -> u8;

    /// Returns the ID of the client that has established this connection.
    fn get_client(&self) -> &ClientPublicKey;

    fn get_compression_set(&self) -> CompressionAlgoSet;
}
