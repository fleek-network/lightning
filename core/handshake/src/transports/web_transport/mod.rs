use std::net::SocketAddr;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use wtransport::endpoint::Server;
use wtransport::Endpoint;

use crate::schema::{HandshakeRequestFrame, HandshakeResponse, RequestFrame, ResponseFrame};
use crate::shutdown::ShutdownWaiter;
use crate::transports::{Transport, TransportReceiver, TransportSender};

#[derive(Deserialize, Serialize)]
pub struct WebTransportConfig {
    address: SocketAddr,
}

impl Default for WebTransportConfig {
    fn default() -> Self {
        Self {
            address: ([0, 0, 0, 0], 4240).into(),
        }
    }
}

pub struct WebTransport {
    endpoint: Endpoint<Server>,
}

#[async_trait]
impl Transport for WebTransport {
    type Config = WebTransportConfig;
    type Sender = WebTransportSender;
    type Receiver = WebTransportReceiver;

    async fn bind(shutdown: ShutdownWaiter, config: Self::Config) -> anyhow::Result<Self> {
        todo!()
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
