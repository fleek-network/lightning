use std::marker::PhantomData;

use async_trait::async_trait;
use lightning_interfaces::{
    schema::LightningMessage, ListenerInterface, SenderReceiver, SignerInterface,
    SyncQueryRunnerInterface,
};

use crate::pool::ConnectionPool;

pub struct Listener<Q, S, T> {
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
        todo!()
    }
}
