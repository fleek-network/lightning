use std::net::SocketAddr;

use async_trait::async_trait;
use bytes::Bytes;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net;
use tokio::net::tcp::{OwnedReadHalf, OwnedWriteHalf};

use crate::transport::{Transport, TransportReceiver, TransportSender};

pub struct TcpTransport {
    target: SocketAddr,
}

impl TcpTransport {
    pub fn new(target: SocketAddr) -> Self {
        Self { target }
    }
}

#[async_trait]
impl Transport for TcpTransport {
    type Sender = TcpSender;
    type Receiver = TcpReceiver;

    async fn connect(&self) -> anyhow::Result<(Self::Sender, Self::Receiver)> {
        let stream = net::TcpStream::connect(self.target).await?;
        let (reader, writer) = stream.into_split();
        Ok((TcpSender { inner: writer }, TcpReceiver { inner: reader }))
    }
}

pub struct TcpSender {
    inner: OwnedWriteHalf,
}

#[async_trait]
impl TransportSender for TcpSender {
    async fn send(&mut self, data: &[u8]) -> anyhow::Result<()> {
        self.inner.write_all(data).await.map_err(Into::into)
    }
}

pub struct TcpReceiver {
    inner: OwnedReadHalf,
}

#[async_trait]
impl TransportReceiver for TcpReceiver {
    async fn recv(&mut self) -> anyhow::Result<Bytes> {
        // Todo: read length prefix.
        let mut buffer = Vec::new();
        self.inner.read_to_end(buffer.as_mut()).await?;
        Ok(Bytes::from(buffer))
    }
}
