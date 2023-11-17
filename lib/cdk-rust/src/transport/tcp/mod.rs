use std::net::SocketAddr;

use anyhow::Result;
use async_trait::async_trait;
use bytes::Bytes;
use futures::StreamExt;
use tokio::io::AsyncWriteExt;
use tokio::net;
use tokio::net::tcp::{OwnedReadHalf, OwnedWriteHalf};
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
impl Transport for TcpTransport {
    type Sender = TcpSender;
    type Receiver = TcpReceiver;

    async fn connect(&self) -> anyhow::Result<(Self::Sender, Self::Receiver)> {
        let stream = net::TcpStream::connect(self.target).await?;
        let (reader, writer) = stream.into_split();
        Ok((
            TcpSender { inner: writer },
            TcpReceiver {
                inner: FramedRead::new(reader, LengthDelimitedCodec::new()),
            },
        ))
    }
}

pub struct TcpSender {
    inner: OwnedWriteHalf,
}

#[async_trait]
impl TransportSender for TcpSender {
    async fn send(&mut self, data: &[u8]) -> Result<()> {
        let frame = transport::create_frame(data);
        self.inner
            .write_all(frame.as_ref())
            .await
            .map_err(Into::into)
    }
}

pub struct TcpReceiver {
    inner: FramedRead<OwnedReadHalf, LengthDelimitedCodec>,
}

#[async_trait]
impl TransportReceiver for TcpReceiver {
    async fn recv(&mut self) -> Option<Bytes> {
        // Todo: log error.
        let bytes = self.inner.next().await?.ok()?;
        Some(bytes.into())
    }
}
