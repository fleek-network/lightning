use std::marker::PhantomData;

use async_trait::async_trait;
use fleek_crypto::NodePublicKey;
use lightning_interfaces::{
    schema::LightningMessage, types::ServiceScope, ReceiverInterface, SenderInterface,
};

use super::schema::ScopedFrame;

pub struct Receiver<T> {
    _t: PhantomData<T>,
    pubkey: NodePublicKey,
    receiver: tokio::sync::mpsc::Receiver<Vec<u8>>,
}
impl<T> Receiver<T> {
    pub(crate) fn new(
        pubkey: NodePublicKey,
        receiver: tokio::sync::mpsc::Receiver<Vec<u8>>,
    ) -> Self {
        Self {
            _t: PhantomData,
            pubkey,
            receiver,
        }
    }
}

#[async_trait]
impl<T: LightningMessage> ReceiverInterface<T> for Receiver<T> {
    fn pk(&self) -> &NodePublicKey {
        &self.pubkey
    }

    async fn recv(&mut self) -> Option<T> {
        self.receiver
            .recv()
            .await
            // TODO: properly propagate decoding errors
            .map(|bytes| T::decode(&bytes).expect("failed to decode payload"))
    }
}

pub struct Sender<T> {
    _t: PhantomData<T>,
    pubkey: NodePublicKey,
    scope: ServiceScope,
    sender: tokio::sync::mpsc::Sender<ScopedFrame>,
}

impl<T> Sender<T> {
    pub(crate) fn new(
        scope: ServiceScope,
        pubkey: NodePublicKey,
        sender: tokio::sync::mpsc::Sender<ScopedFrame>,
    ) -> Self {
        Self {
            _t: PhantomData,
            pubkey,
            scope,
            sender,
        }
    }
}

#[async_trait]
impl<T: LightningMessage> SenderInterface<T> for Sender<T> {
    fn pk(&self) -> &NodePublicKey {
        &self.pubkey
    }

    async fn send(&self, msg: T) -> bool {
        let mut payload = Vec::new();
        msg.encode(&mut payload).expect("failed to encode message");
        self.sender
            .send(ScopedFrame::Message {
                scope: self.scope,
                payload,
            })
            .await
            .is_ok()
    }
}
