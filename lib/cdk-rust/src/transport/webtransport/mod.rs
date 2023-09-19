use std::pin::Pin;
use std::task::{Context, Poll};

use async_trait::async_trait;
use bytes::Bytes;
use futures::{Sink, Stream};
use tokio_util::codec::{FramedRead, FramedWrite, LengthDelimitedCodec};
use wtransport::{RecvStream, SendStream};

use crate::transport::Transport;

pub struct WebTransport;

pub type FramedStreamTx = FramedWrite<SendStream, LengthDelimitedCodec>;
pub type FramedStreamRx = FramedRead<RecvStream, LengthDelimitedCodec>;

#[async_trait]
impl Transport for WebTransport {
    type Sender = WebTransportSender;
    type Receiver = WebTransportReceiver;

    async fn connect(&self) -> anyhow::Result<(Self::Sender, Self::Receiver)> {
        todo!()
    }
}

pub struct WebTransportSender {
    inner: FramedStreamTx,
}

impl Sink<Bytes> for WebTransportSender {
    type Error = std::io::Error;

    fn poll_ready(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Pin::new(&mut self.inner)
            .poll_ready(cx)
            .map_err(std::io::Error::from)
    }

    fn start_send(mut self: Pin<&mut Self>, item: Bytes) -> Result<(), Self::Error> {
        Pin::new(&mut self.inner)
            .start_send(item)
            .map_err(std::io::Error::from)
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Pin::new(&mut self.inner)
            .poll_flush(cx)
            .map_err(std::io::Error::from)
    }

    fn poll_close(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Pin::new(&mut self.inner)
            .poll_close(cx)
            .map_err(std::io::Error::from)
    }
}

pub struct WebTransportReceiver {
    inner: FramedStreamRx,
}

impl Stream for WebTransportReceiver {
    type Item = Bytes;

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
