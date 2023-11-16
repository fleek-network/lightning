use std::net::SocketAddr;

use async_trait::async_trait;
use bytes::Bytes;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net;
use tokio::net::tcp::{OwnedReadHalf, OwnedWriteHalf};
use tokio::sync::Mutex;

use crate::transport::{Transport, TransportStream};

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
    type Stream = TcpStream;

    async fn connect(&self) -> anyhow::Result<Self::Stream> {
        let stream = net::TcpStream::connect(self.target).await?;
        let (reader, writer) = stream.into_split();
        Ok(TcpStream {
            reader,
            writer: Mutex::new(Some(writer)),
        })
    }
}

pub struct TcpStream {
    reader: OwnedReadHalf,
    // Todo: This is temporary.
    writer: Mutex<Option<OwnedWriteHalf>>,
}

#[async_trait]
impl TransportStream for TcpStream {
    async fn send(&self, data: &[u8]) -> anyhow::Result<()> {
        let mut guard = self.writer.lock().await;
        let writer = guard.as_mut().take().unwrap();
        writer.write_all(data).await.map_err(Into::into)
    }

    async fn recv(&mut self) -> anyhow::Result<Bytes> {
        let mut buffer = Vec::new();
        self.reader.read_to_end(buffer.as_mut()).await?;
        Ok(Bytes::from(buffer))
    }
}
