use std::marker::PhantomData;

use async_trait::async_trait;
use fleek_crypto::NodePublicKey;
use lightning_interfaces::schema::LightningMessage;
use lightning_interfaces::ListenerInterface;
use quinn::{RecvStream, SendStream};
use tokio::sync::mpsc;

use crate::receiver::Receiver;
use crate::sender::Sender;

/// Pool listener.
pub struct Listener<T> {
    /// Receiver of new connection streams.
    connection_event_rx: mpsc::Receiver<(NodePublicKey, SendStream, RecvStream)>,
    _marker: PhantomData<T>,
}

impl<T> Listener<T> {
    pub fn new(
        connection_event_rx: mpsc::Receiver<(NodePublicKey, SendStream, RecvStream)>,
    ) -> Self {
        Self {
            connection_event_rx,
            _marker: PhantomData,
        }
    }
}

impl<T> Drop for Listener<T> {
    fn drop(&mut self) {
        self.connection_event_rx.close();
    }
}

#[async_trait]
impl<T> ListenerInterface<T> for Listener<T>
where
    T: LightningMessage,
{
    type Sender = Sender<T>;
    type Receiver = Receiver<T>;
    async fn accept(&mut self) -> Option<(Self::Sender, Self::Receiver)> {
        let (peer, tx, rx) = self.connection_event_rx.recv().await?;
        Some((Sender::new(tx, peer), Receiver::new(rx, peer)))
    }
}
