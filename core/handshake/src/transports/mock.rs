use async_trait::async_trait;
use bytes::Bytes;
use dashmap::DashMap;
use log::error;
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc::{channel, Receiver, Sender};
use tokio::sync::OnceCell;

use super::{Transport, TransportReceiver, TransportSender};
use crate::schema;
use crate::shutdown::ShutdownWaiter;

static LISTENERS: OnceCell<
    DashMap<u16, tokio::sync::mpsc::Sender<(MockTransportSender, MockTransportReceiver)>>,
> = OnceCell::const_new();

/// Dial a mocked connection, returning a tokio sender and receiver for the "client".
/// Users are in charge of sending the initial [`HandshakeRequestFrame`].
pub async fn dial_mock(port: u16) -> Option<(Sender<Bytes>, Receiver<Bytes>)> {
    let map = LISTENERS.get_or_init(|| async { DashMap::new() }).await;
    let conn_tx = map.get(&port)?;

    let (tx1, rx2) = channel(16);
    let (tx2, rx1) = channel(16);
    conn_tx
        .send((
            MockTransportSender { tx: tx1 },
            MockTransportReceiver { rx: rx1 },
        ))
        .await
        .ok()?;

    Some((tx2, rx2))
}

/// Mock memory transport backed by tokio channels
pub struct MockTransport {
    conn_rx: tokio::sync::mpsc::Receiver<(MockTransportSender, MockTransportReceiver)>,
}

#[derive(Default, Serialize, Deserialize)]
pub struct MockTransportConfig {
    port: u16,
}

#[async_trait]
impl Transport for MockTransport {
    type Config = MockTransportConfig;
    type Sender = MockTransportSender;
    type Receiver = MockTransportReceiver;

    async fn bind(_waiter: ShutdownWaiter, config: Self::Config) -> anyhow::Result<Self> {
        let (conn_tx, conn_rx) = tokio::sync::mpsc::channel(8);

        let map = LISTENERS.get_or_init(|| async { DashMap::new() }).await;
        assert!(!map.contains_key(&config.port));
        map.insert(config.port, conn_tx);

        Ok(Self { conn_rx })
    }

    async fn accept(
        &mut self,
    ) -> Option<(schema::HandshakeRequestFrame, Self::Sender, Self::Receiver)> {
        let (sender, mut receiver) = self.conn_rx.recv().await?;
        let bytes = receiver.rx.recv().await?;
        let frame = schema::HandshakeRequestFrame::decode(&bytes).ok()?;

        Some((frame, sender, receiver))
    }
}

/// Mock sender
pub struct MockTransportSender {
    tx: tokio::sync::mpsc::Sender<Bytes>,
}

macro_rules! mock_send {
    ($t1:expr, $t2:expr) => {
        let sender = $t1.tx.clone();
        let bytes = $t2.encode();
        tokio::spawn(async move {
            if let Err(e) = sender.send(bytes).await {
                error!("failed to send message to client: {e}");
            };
        });
    };
}

impl TransportSender for MockTransportSender {
    fn send_handshake_response(&mut self, response: schema::HandshakeResponse) {
        mock_send!(self, response);
    }

    fn send(&mut self, frame: schema::ResponseFrame) {
        mock_send!(self, frame);
    }
}

/// Mock receiver
pub struct MockTransportReceiver {
    rx: tokio::sync::mpsc::Receiver<Bytes>,
}

#[async_trait]
impl TransportReceiver for MockTransportReceiver {
    async fn recv(&mut self) -> Option<schema::RequestFrame> {
        let bytes = self.rx.recv().await?;
        schema::RequestFrame::decode(&bytes).ok()
    }
}

#[cfg(test)]
mod tests {
    use fleek_crypto::{ClientPublicKey, ClientSignature};

    use super::*;
    use crate::shutdown::ShutdownNotifier;

    #[tokio::test]
    async fn dial() -> anyhow::Result<()> {
        let notifier = ShutdownNotifier::default();
        let mut server =
            MockTransport::bind(notifier.waiter(), MockTransportConfig { port: 420 }).await?;

        let client = dial_mock(420).await.unwrap();
        // send the initial handshake
        client
            .0
            .send(
                schema::HandshakeRequestFrame::Handshake {
                    retry: None,
                    service: 0,
                    pk: ClientPublicKey([1; 96]),
                    pop: ClientSignature([2; 48]),
                }
                .encode(),
            )
            .await?;

        assert!(server.accept().await.is_some());

        Ok(())
    }
}
