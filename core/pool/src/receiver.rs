use std::marker::PhantomData;

use async_trait::async_trait;
use fleek_crypto::NodePublicKey;
use lightning_interfaces::{schema::LightningMessage, ReceiverInterface};
use quinn::{Connection, RecvStream};

pub struct Receiver<T> {
    receive: RecvStream,
    _marker: PhantomData<T>,
}

impl<T> Receiver<T>
where
    T: LightningMessage,
{
    pub fn new(receive: RecvStream) -> Self {
        Self {
            receive,
            _marker: PhantomData::default(),
        }
    }
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
