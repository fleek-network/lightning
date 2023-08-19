use std::{marker::PhantomData, net::SocketAddr};

use async_trait::async_trait;
use fleek_crypto::NodePublicKey;
use lightning_interfaces::{
    schema::LightningMessage, types::ServiceScope, ConnectorInterface, SenderReceiver,
    SignerInterface, SyncQueryRunnerInterface,
};
use quinn::{Connection, RecvStream, SendStream};
use tokio::sync::{mpsc, oneshot};

use crate::{pool::ConnectionPool, receiver::Receiver, sender::Sender};

pub struct ConnectEvent {
    pub scope: ServiceScope,
    pub pk: NodePublicKey,
    pub address: SocketAddr,
    pub respond: oneshot::Sender<(SendStream, RecvStream)>,
}

pub struct Connector<Q, S, T> {
    scope: ServiceScope,
    connection_event_tx: mpsc::Sender<ConnectEvent>,
    _marker: PhantomData<(Q, S, T)>,
}

impl<Q, S, T> Clone for Connector<Q, S, T> {
    fn clone(&self) -> Self {
        todo!()
    }
}

#[async_trait]
impl<Q, S, T> ConnectorInterface<T> for Connector<Q, S, T>
where
    T: LightningMessage,
    Q: SyncQueryRunnerInterface,
    S: SignerInterface,
{
    type ConnectionPool = ConnectionPool<Q, S>;

    async fn connect(&self, to: &NodePublicKey) -> Option<SenderReceiver<Self::ConnectionPool, T>> {
        let (tx, rx) = oneshot::channel();
        self.connection_event_tx
            .send(ConnectEvent {
                scope: self.scope,
                pk: *to,
                address: "0.0.0.0:0".parse().unwrap(),
                respond: tx,
            })
            .await
            .ok()?;
        let (tx_stream, rx_stream) = rx.await.ok()?;
        Some((Sender::new(tx_stream), Receiver::new(rx_stream)))
    }
}
