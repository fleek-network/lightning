pub mod tcp;
#[cfg(not(feature = "cloudflare"))]
pub mod webtransport;

use anyhow::Result;
use async_trait::async_trait;
use bytes::{BufMut, Bytes, BytesMut};

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
    async fn recv(&mut self) -> Option<Bytes>;
}

pub fn create_frame(bytes: &[u8]) -> Bytes {
    let mut buf = BytesMut::with_capacity(4 + bytes.len());
    buf.put_u32(bytes.len() as u32);
    buf.put(bytes);
    buf.into()
}
