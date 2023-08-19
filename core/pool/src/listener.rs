use std::marker::PhantomData;

use async_trait::async_trait;
use fleek_crypto::NodePublicKey;
use lightning_interfaces::{
    schema::LightningMessage, ListenerInterface, SenderReceiver, SignerInterface,
    SyncQueryRunnerInterface,
};
use quinn::{Connection, ConnectionError, Endpoint, RecvStream, SendStream};
use tokio::sync::mpsc;

use crate::{connection::RegisterEvent, pool::ConnectionPool, receiver::Receiver, sender::Sender};

pub struct Listener<Q, S, T> {
    connection_event_rx: mpsc::Receiver<Option<(NodePublicKey, SendStream, RecvStream)>>,
    _marker: PhantomData<(Q, S, T)>,
}

#[async_trait]
impl<Q, S, T> ListenerInterface<T> for Listener<Q, S, T>
where
    T: LightningMessage,
    Q: SyncQueryRunnerInterface,
    S: SignerInterface,
{
    type ConnectionPool = ConnectionPool<Q, S>;

    async fn accept(&mut self) -> Option<SenderReceiver<Self::ConnectionPool, T>> {
        let (peer, tx, rx) = self.connection_event_rx.recv().await.flatten()?;
        Some((Sender::new(tx, peer), Receiver::new(rx, peer)))
    }
}
