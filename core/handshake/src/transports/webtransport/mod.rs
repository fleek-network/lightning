mod server;

use std::net::SocketAddr;
use std::time::Duration;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;
use tokio::sync::mpsc::Receiver;
use wtransport::endpoint::Server;
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
        let mut config = ServerConfig::builder()
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
        todo!()
    }
}

pub struct WebTransportSender;

impl TransportSender for WebTransportSender {
    fn send_handshake_response(&mut self, response: HandshakeResponse) {
        todo!()
    }

    fn send(&mut self, frame: ResponseFrame) {
        todo!()
    }
}

pub struct WebTransportReceiver;

#[async_trait]
impl TransportReceiver for WebTransportReceiver {
    async fn recv(&mut self) -> Option<RequestFrame> {
        todo!()
    }
}
