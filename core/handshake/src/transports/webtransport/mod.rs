mod connection;
mod server;

use std::io::Error;
use std::net::SocketAddr;
use std::time::Duration;

use async_trait::async_trait;
use bytes::BytesMut;
use futures::StreamExt;
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc::{self, Receiver, Sender};
use tokio_util::codec::{FramedRead, FramedWrite, LengthDelimitedCodec};
use wtransport::tls::Certificate;
use wtransport::{Connection, Endpoint, RecvStream, SendStream, ServerConfig};

use crate::schema::{HandshakeRequestFrame, HandshakeResponse, RequestFrame, ResponseFrame};
use crate::shutdown::ShutdownWaiter;
use crate::transports::webtransport::server::Context;
use crate::transports::{Transport, TransportReceiver, TransportSender};

#[derive(Deserialize, Serialize)]
pub struct WebTransportConfig {
    address: SocketAddr,
    keep_alive: Option<Duration>,
}

impl Default for WebTransportConfig {
    fn default() -> Self {
        Self {
            address: ([0, 0, 0, 0], 4240).into(),
            keep_alive: None,
        }
    }
}

pub struct WebTransport {
    conn_rx: Receiver<(SendStream, RecvStream)>,
}

#[async_trait]
impl Transport for WebTransport {
    type Config = WebTransportConfig;
    type Sender = WebTransportSender;
    type Receiver = WebTransportReceiver;

    async fn bind(shutdown: ShutdownWaiter, config: Self::Config) -> anyhow::Result<Self> {
        let config = ServerConfig::builder()
            .with_bind_address(config.address)
            .with_certificate(Certificate::new(vec![], vec![]))
            .keep_alive_interval(config.keep_alive)
            .build();

        let endpoint = Endpoint::server(config)?;
        let (conn_tx, conn_rx) = mpsc::channel(2048);
        let ctx = Context {
            endpoint,
            conn_tx,
            shutdown,
        };
        tokio::spawn(server::main_loop(ctx));

        Ok(Self { conn_rx })
    }

    async fn accept(&mut self) -> Option<(HandshakeRequestFrame, Self::Sender, Self::Receiver)> {
        let (tx, mut rx) = self.conn_rx.recv().await?;
        let frame = match HandshakeRequestFrame::decode_from_reader(&mut rx).await {
            Ok(f) => f,
            Err(e) => {
                log::error!("failed to get handshake request frame {e:?}");
                return None;
            },
        };
        let frame_writer = FramedWrite::new(tx, LengthDelimitedCodec::new());
        let (data_tx, data_rx) = mpsc::channel(2048);
        tokio::spawn(connection::sender_loop(data_rx, frame_writer));
        let frame_reader = FramedRead::new(rx, LengthDelimitedCodec::new());
        Some((
            frame,
            WebTransportSender { tx: data_tx },
            WebTransportReceiver { rx: frame_reader },
        ))
    }
}

pub struct WebTransportSender {
    tx: Sender<Vec<u8>>,
}

impl TransportSender for WebTransportSender {
    fn send_handshake_response(&mut self, response: HandshakeResponse) {
        let tx = self.tx.clone();
        let data = response.encode();
        tokio::spawn(async move {
            if tx.send(data.to_vec()).await.is_err() {
                log::error!("failed to send data to connection loop");
            }
        });
    }

    fn send(&mut self, frame: ResponseFrame) {
        let tx = self.tx.clone();
        let data = frame.encode();
        tokio::spawn(async move {
            if tx.send(data.to_vec()).await.is_err() {
                log::error!("failed to send data to connection loop");
            }
        });
    }
}

pub struct WebTransportReceiver {
    rx: FramedRead<RecvStream, LengthDelimitedCodec>,
}

#[async_trait]
impl TransportReceiver for WebTransportReceiver {
    async fn recv(&mut self) -> Option<RequestFrame> {
        let data = match self.rx.next().await? {
            Ok(data) => data,
            Err(e) => {
                log::error!("failed to get next frame: {e:?}");
                return None;
            },
        };
        match RequestFrame::decode(&data) {
            Ok(data) => Some(data),
            Err(e) => {
                log::error!("failed to decode request frame: {e:?}");
                None
            },
        }
    }
}
