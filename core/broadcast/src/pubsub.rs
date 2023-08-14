use std::{marker::PhantomData, sync::Arc};

use async_trait::async_trait;
use dashmap::DashMap;
use lightning_interfaces::{schema::LightningMessage, PubSub, Topic};

pub struct PubSubTopic<M: LightningMessage> {
    _m: PhantomData<M>,
    topic: Topic,
    /// sender for outgoing messages
    sender: tokio::sync::mpsc::Sender<(Topic, Vec<u8>)>,
    /// receiver for incoming messages
    receiver: tokio::sync::broadcast::Receiver<Vec<u8>>,
    /// map of broadcast senders. Needed to implement Clone.
    channels: Arc<DashMap<Topic, tokio::sync::broadcast::Sender<Vec<u8>>>>,
}

impl<M: LightningMessage> PubSubTopic<M> {
    pub(crate) fn new(
        topic: Topic,
        sender: tokio::sync::mpsc::Sender<(Topic, Vec<u8>)>,
        receiver: tokio::sync::broadcast::Receiver<Vec<u8>>,
        channels: Arc<DashMap<Topic, tokio::sync::broadcast::Sender<Vec<u8>>>>,
    ) -> Self {
        Self {
            _m: PhantomData,
            topic,
            sender,
            receiver,
            channels,
        }
    }
}

impl<M: LightningMessage> Clone for PubSubTopic<M> {
    fn clone(&self) -> Self {
        Self {
            _m: self._m,
            topic: self.topic,
            sender: self.sender.clone(),
            receiver: self
                .channels
                .get(&self.topic)
                .expect("failed to find sender")
                .subscribe(),
            channels: self.channels.clone(),
        }
    }
}

// TODO: adjust interface so the outputs are wrapped in a result, and can be infallible.
#[async_trait]
impl<M> PubSub<M> for PubSubTopic<M>
where
    M: LightningMessage + Clone,
{
    async fn send(&self, msg: &M) {
        let mut payload = Vec::new();
        msg.encode(&mut payload).expect("failed to encode content");
        self.sender
            .send((self.topic, payload))
            .await
            .expect("failed to broadcast message");
    }

    async fn recv(&mut self) -> Option<M> {
        self.receiver
            .recv()
            .await
            .ok()
            .map(|bytes| M::decode(&bytes).expect("failed to decode pubsub message"))
    }
}
