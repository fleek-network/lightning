use std::marker::PhantomData;

use async_trait::async_trait;
use fleek_crypto::NodePublicKey;
use lightning_interfaces::{schema::LightningMessage, SenderInterface};
use quinn::{Connection, SendStream};

pub struct Sender<T> {
    send: SendStream,
    _marker: PhantomData<T>,
}

impl<T> Sender<T>
where
    T: LightningMessage,
{
    pub fn new(send: SendStream) -> Self {
        Self {
            send,
            _marker: PhantomData::default(),
        }
    }
}

#[async_trait]
impl<T> SenderInterface<T> for Sender<T>
where
    T: LightningMessage,
{
    fn pk(&self) -> &NodePublicKey {
        todo!()
    }

    async fn send(&self, msg: T) -> bool {
        todo!()
    }
}
