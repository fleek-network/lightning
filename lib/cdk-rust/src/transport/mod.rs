pub mod tcp;
pub mod webtransport;

use anyhow::Result;
use async_trait::async_trait;
use bytes::Bytes;

#[async_trait]
pub trait Transport: Send + Sync + 'static {
    type Sender: TransportSender;
    type Receiver: TransportReceiver;

    async fn connect(&self) -> Result<(Self::Sender, Self::Receiver)>;
}

#[async_trait]
pub trait TransportSender {
    async fn send(&mut self, data: &[u8]) -> Result<()>;
}

#[async_trait]
pub trait TransportReceiver {
    // This method must be cancel safe.
    async fn recv(&mut self) -> Result<Bytes>;
}
