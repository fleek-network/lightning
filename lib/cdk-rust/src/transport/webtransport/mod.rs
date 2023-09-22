use std::net::SocketAddr;
use std::pin::Pin;
use std::task::{Context, Poll};

use anyhow::Result;
use async_trait::async_trait;
use bytes::Bytes;
use futures::{Sink, Stream};
use tokio_util::codec::{FramedRead, FramedWrite, LengthDelimitedCodec};
use wtransport::endpoint::endpoint_side::Client;
use wtransport::{ClientConfig, Endpoint, RecvStream, SendStream};

use crate::tls;
use crate::transport::Transport;

pub struct WebTransport {
    target: String,
    endpoint: Endpoint<Client>,
}

impl WebTransport {
    pub fn new(config: Config) -> Result<Self> {
        let client_config = ClientConfig::builder()
            .with_bind_address(config.bind_address)
            // Todo: We need to customize handshake to validate server certificate hashes.
            // Maybe we don't have to do this if we perform authentication using keys
            // in a custom TLS validator similarly to libp2p.
            .with_custom_tls(tls::tls_config(config.server_hashes))
            .build();
        let endpoint = Endpoint::client(client_config)?;
        Ok(Self {
            endpoint,
            target: config.target,
        })
    }
}

pub struct Config {
    pub target: String,
    pub server_hashes: Vec<Vec<u8>>,
    pub bind_address: SocketAddr,
}

pub type FramedStreamTx = FramedWrite<SendStream, LengthDelimitedCodec>;
pub type FramedStreamRx = FramedRead<RecvStream, LengthDelimitedCodec>;

#[async_trait]
impl Transport for WebTransport {
    type Sender = WebTransportSender;
    type Receiver = WebTransportReceiver;

    async fn connect(&self) -> anyhow::Result<(Self::Sender, Self::Receiver)> {
        let conn = self.endpoint.connect(&self.target).await?;
        // Todo: For now, we just open one bidirectional stream.
        // Ideally, we would like to generate new streams from the connection
        // and perform a request-response exchange in each stream.
        let (tx, rx) = conn.open_bi().await?.await?;
        Ok((
            WebTransportSender {
                inner: FramedStreamTx::new(tx, LengthDelimitedCodec::new()),
            },
            WebTransportReceiver {
                inner: FramedStreamRx::new(rx, LengthDelimitedCodec::new()),
            },
        ))
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
