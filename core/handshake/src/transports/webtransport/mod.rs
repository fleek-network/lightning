mod connection;
mod server;

use std::net::SocketAddr;
use std::time::Duration;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc::{self, Receiver, Sender};
use wtransport::tls::Certificate;
use wtransport::{Connection, Endpoint, ServerConfig};

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
    conn_rx: Receiver<Connection>,
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
        let conn = self.conn_rx.recv().await?;
        let rx = match conn.accept_uni().await {
            Ok(rx) => rx,
            Err(e) => {
                log::error!("failed to establish first stream: {e:?}");
                return None;
            },
        };
        let frame = match HandshakeRequestFrame::decode_from_reader(rx).await {
            Ok(f) => f,
            Err(e) => {
                log::error!("failed to get handshake request frame {e:?}");
                return None;
            },
        };
        let (sender_tx, sender_rx) = mpsc::channel(1024);
        let (receiver_tx, receiver_rx) = mpsc::channel(1024);

        let context = connection::Context::new(conn, receiver_tx, sender_rx);
        tokio::spawn(async move {
            if let Err(e) = connection::connection_loop(context).await {
                log::error!("unexpected error from connection loop: {e:?}");
            }
        });
        Some((
            frame,
            WebTransportSender { tx: sender_tx },
            WebTransportReceiver { rx: receiver_rx },
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
    rx: Receiver<Vec<u8>>,
}

#[async_trait]
impl TransportReceiver for WebTransportReceiver {
    async fn recv(&mut self) -> Option<RequestFrame> {
        let data = self.rx.recv().await?;
        match RequestFrame::decode(&data) {
            Ok(data) => Some(data),
            Err(e) => {
                log::error!("failed to decode request frame: {e:?}");
                None
            },
        }
    }
}
