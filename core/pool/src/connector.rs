use std::marker::PhantomData;

use async_trait::async_trait;
use quinn::Connection;
use tokio::sync::mpsc::Sender;
use tokio::sync::oneshot;
use lightning_interfaces::{
    schema::LightningMessage, ConnectorInterface, SenderReceiver, SignerInterface,
    SyncQueryRunnerInterface,
};

use crate::pool::ConnectionPool;

/// ConnectionCommand
pub struct NewConnectionCommand {
    peer: fleek_crypto::NodePublicKey,
    respond: oneshot::Sender<Option<Connection>>,
}

pub struct Connector<Q, S, T> {
    pool_request_tx: Sender<NewConnectionCommand>,
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

    async fn connect(
        &self,
        to: &fleek_crypto::NodePublicKey,
    ) -> Option<SenderReceiver<Self::ConnectionPool, T>> {
        todo!()
    }
}
