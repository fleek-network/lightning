use std::net::SocketAddr;

use anyhow::Result;
use async_trait::async_trait;
use bytes::Bytes;
use futures::StreamExt;
use tokio::io::{AsyncRead, AsyncWrite, AsyncWriteExt};
#[cfg(feature = "cloudflare")]
use tokio::io::{ReadHalf, WriteHalf};
#[cfg(not(feature = "cloudflare"))]
use tokio::net::{
    self,
    tcp::{OwnedReadHalf, OwnedWriteHalf},
};
use tokio_util::codec::{FramedRead, LengthDelimitedCodec};

use crate::transport;
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
#[cfg(not(feature = "cloudflare"))]
impl Transport for TcpTransport {
    type Sender = TcpSender<OwnedWriteHalf>;
    type Receiver = TcpReceiver<OwnedReadHalf>;

    async fn connect(&self) -> anyhow::Result<(Self::Sender, Self::Receiver)> {
        let stream = net::TcpStream::connect(self.target).await?;
        let (reader, writer) = stream.into_split();

        Ok((TcpSender::new(writer), TcpReceiver::new(reader)))
    }
}

#[async_trait]
#[cfg(feature = "cloudflare")]
impl Transport for TcpTransport {
    type Sender = TcpSender<WriteHalf<worker::Socket>>;
    type Receiver = TcpReceiver<ReadHalf<worker::Socket>>;

    async fn connect(&self) -> anyhow::Result<(Self::Sender, Self::Receiver)> {
        let socket = worker::Socket::builder()
            .connect(self.target.ip().to_string(), self.target.port())
            .map_err(|e| anyhow::anyhow!("failed to connect: {:?}", e))?;

        let (reader, writer) = tokio::io::split(socket);

        Ok((TcpSender::new(writer), TcpReceiver::new(reader)))
    }
}

pub struct TcpSender<W> {
    inner: W,
}

impl<S> TcpSender<S> {
    pub fn new(inner: S) -> Self {
        Self { inner }
    }
}

#[async_trait]
impl<W: AsyncWrite + Unpin + Send + Sync> TransportSender for TcpSender<W> {
    async fn send(&mut self, data: &[u8]) -> Result<()> {
        let frame = transport::create_frame(data);
        self.inner
            .write_all(frame.as_ref())
            .await
            .map_err(Into::into)
    }
}

pub struct TcpReceiver<R: AsyncRead> {
    inner: FramedRead<R, LengthDelimitedCodec>,
}

impl<R: AsyncRead> TcpReceiver<R> {
    pub fn new(inner: R) -> Self {
        Self {
            inner: FramedRead::new(inner, LengthDelimitedCodec::new()),
        }
    }
}

#[async_trait]
impl<R: AsyncRead + Unpin + Send + Sync> TransportReceiver for TcpReceiver<R> {
    async fn recv(&mut self) -> Option<Bytes> {
        // Todo: log error.
        let bytes = self.inner.next().await?.ok()?;
        Some(bytes.into())
    }
}
