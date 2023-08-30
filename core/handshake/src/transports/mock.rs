use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use super::{Transport, TransportReceiver, TransportSender};
use crate::schema;

pub struct MockTransport {}

pub struct MockTransportSender {}

pub struct MockTransportReceiver {}

#[derive(Default, Serialize, Deserialize)]
pub struct MockTransportConfig {}

#[async_trait]
impl Transport for MockTransport {
    type Config = MockTransportConfig;

    type Sender = MockTransportSender;
    type Receiver = MockTransportReceiver;

    async fn bind(_config: Self::Config) -> anyhow::Result<Self> {
        Ok(Self {})
    }

    async fn accept(
        &mut self,
    ) -> Option<(schema::HandshakeRequestFrame, Self::Sender, Self::Receiver)> {
        todo!()
    }
}

impl TransportSender for MockTransportSender {
    fn send_handshake_response(&mut self, _response: schema::HandshakeResponse) {
        todo!()
    }

    fn send(&mut self, _frame: schema::ResponseFrame) {
        todo!()
    }
}

#[async_trait]
impl TransportReceiver for MockTransportReceiver {
    async fn recv(&mut self) -> Option<schema::RequestFrame> {
        todo!()
    }
}
