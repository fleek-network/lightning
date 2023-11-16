pub mod webtransport;

use anyhow::Result;
use async_trait::async_trait;
use bytes::Bytes;

#[async_trait]
pub trait Transport: Send + Sync + 'static {
    type Stream: TransportStream;

    async fn connect(&self) -> Result<Self::Stream>;
}

#[async_trait]
pub trait TransportStream {
    async fn send(&self, data: &[u8]) -> Result<()>;

    // This method must be cancel safe.
    async fn recv(&mut self) -> Result<Bytes>;
}
