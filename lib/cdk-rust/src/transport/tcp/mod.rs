use std::io;
use std::net::SocketAddr;
use std::pin::Pin;
use std::task::{Context, Poll};

use async_trait::async_trait;
use bytes::Bytes;
use futures::{Sink, Stream};
use tokio::net::tcp::{OwnedReadHalf, OwnedWriteHalf};
use tokio::net::TcpStream;
use tokio_util::codec::{FramedRead, FramedWrite, LengthDelimitedCodec};

use crate::transport::{Message, Transport};

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
        let stream = TcpStream::connect(self.target).await?;
        let (rx, tx) = stream.into_split();
        Ok((
            TcpSender {
                inner: FramedWrite::new(tx, LengthDelimitedCodec::new()),
            },
            TcpReceiver {
                inner: FramedRead::new(rx, LengthDelimitedCodec::new()),
            },
        ))
    }
}

pub struct TcpSender {
    inner: FramedWrite<OwnedWriteHalf, LengthDelimitedCodec>,
}

impl Sink<Message> for TcpSender {
    type Error = io::Error;

    fn poll_ready(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Pin::new(&mut self.inner).poll_ready(cx)
    }

    fn start_send(mut self: Pin<&mut Self>, item: Message) -> Result<(), Self::Error> {
        Pin::new(&mut self.inner).start_send(item)
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Pin::new(&mut self.inner).poll_flush(cx)
    }

    fn poll_close(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Pin::new(&mut self.inner).poll_close(cx)
    }
}

pub struct TcpReceiver {
    inner: FramedRead<OwnedReadHalf, LengthDelimitedCodec>,
}

impl Stream for TcpReceiver {
    type Item = Message;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        Pin::new(&mut self.inner)
            .poll_next(cx)
            .map(|opt| match opt {
                None => None,
                Some(Ok(bytes)) => Some(Bytes::from(bytes)),
                Some(Err(e)) => {
                    log::error!("unexpected error in receiving stream: {e:?}");
                    None
                },
            })
    }
}
