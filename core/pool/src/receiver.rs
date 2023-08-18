use std::marker::PhantomData;

use async_trait::async_trait;
use fleek_crypto::NodePublicKey;
use lightning_interfaces::{schema::LightningMessage, ReceiverInterface};
use quinn::Connection;

pub struct Receiver<T> {
    connection: Connection,
    pk: NodePublicKey,
    _marker: PhantomData<T>,
}

#[async_trait]
impl<T> ReceiverInterface<T> for Receiver<T>
where
    T: LightningMessage,
{
    fn pk(&self) -> &NodePublicKey {
        todo!()
    }

    async fn recv(&mut self) -> Option<T> {
        todo!()
    }
}
