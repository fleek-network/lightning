use std::collections::HashSet;
use std::marker::PhantomData;

use async_trait::async_trait;
use lightning_interfaces::schema::LightningMessage;
use lightning_interfaces::types::{Digest, NodeIndex, Topic};
use lightning_interfaces::{BroadcastEventInterface, PubSub};
use tokio::sync::oneshot;
use tracing::{debug, info};

use crate::command::{Command, CommandSender, RecvCmd, SendCmd};

pub struct PubSubI<T: LightningMessage + Clone> {
    topic: Topic,
    command_sender: CommandSender,
    last_seen: Option<usize>,
    is_alive: bool,
    message: PhantomData<T>,
}

pub struct Event<T> {
    digest: Digest,
    message: Option<T>,
    originator: NodeIndex,
    command_sender: CommandSender,
}

impl<T: LightningMessage + Clone> PubSubI<T> {
    pub(crate) fn new(topic: Topic, command_sender: CommandSender) -> Self {
        Self {
            topic,
            command_sender,
            last_seen: None,
            is_alive: true,
            message: PhantomData,
        }
    }
}

impl<T: LightningMessage + Clone> Clone for PubSubI<T> {
    fn clone(&self) -> Self {
        Self {
            topic: self.topic,
            command_sender: self.command_sender.clone(),
            last_seen: self.last_seen,
            is_alive: self.is_alive,
            message: PhantomData,
        }
    }
}

#[async_trait]
impl<T: LightningMessage + Clone> PubSub<T> for PubSubI<T> {
    type Event = Event<T>;

    async fn send(&self, msg: &T, filter: Option<HashSet<NodeIndex>>) {
        debug!("sending a message on topic {:?}", self.topic);

        let mut payload = Vec::with_capacity(512);
        msg.encode(&mut payload)
            .expect("Unexpected failure writing to buffer.");
        let _ = self.command_sender.send(Command::Send(SendCmd {
            topic: self.topic,
            filter,
            payload,
        }));
    }

    /// Propagate a message that we already propagated before.
    async fn repropagate(&self, digest: Digest) {
        debug!("repropagate a message on topic {:?}", self.topic);
        let _ = self.command_sender.send(Command::Propagate(digest));
    }

    /// Receive the oldest message we still haven't seen by this receiver. Due to the ring-buf like
    /// behavior of the broadcast. We may miss messages if there is a very long delay in between
    /// calls to recv.
    async fn recv(&mut self) -> Option<T> {
        debug!("brodcast::recv called on topic {:?}", self.topic);

        if !self.is_alive {
            return None;
        }

        loop {
            let (tx, rx) = oneshot::channel();
            let _ = self.command_sender.send(Command::Recv(RecvCmd {
                topic: self.topic,
                last_seen: self.last_seen,
                response: tx,
            }));

            // Wait for the response. If we get an error just return `None`. We're never going to
            // get anything again.
            debug!("awaiting for a message in topic {:?}", self.topic);

            let Ok((id, msg)) = rx.await else {
                self.is_alive = false;
                return None;
            };

            // Store the index of the last message we saw.
            self.last_seen = Some(id);

            if let Ok(decoded) = T::decode(&msg.payload) {
                let _ = self.command_sender.send(Command::Propagate(msg.digest));
                return Some(decoded);
            } else {
                info!(
                    "received an invalid message which we couldnt not deserialize {:?}",
                    msg
                );
            }
        }
    }

    async fn recv_event(&mut self) -> Option<Self::Event> {
        debug!("brodcast::recv_event called on topic {:?}", self.topic);

        if !self.is_alive {
            return None;
        }

        loop {
            let (tx, rx) = oneshot::channel();
            let _ = self.command_sender.send(Command::Recv(RecvCmd {
                topic: self.topic,
                last_seen: self.last_seen,
                response: tx,
            }));

            // Wait for the response. If we get an error just return `None`. We're never going to
            // get anything again.
            debug!("awaiting for a message in topic {:?}", self.topic);

            let Ok((id, msg)) = rx.await else {
                self.is_alive = false;
                return None;
            };

            // Store the index of the last message we saw.
            self.last_seen = Some(id);

            if let Ok(decoded) = T::decode(&msg.payload) {
                let event = Event::<T> {
                    digest: msg.digest,
                    message: Some(decoded),
                    originator: msg.origin,
                    command_sender: self.command_sender.clone(),
                };
                return Some(event);
            } else {
                info!(
                    "received an invalid message which we couldnt not deserialize {:?}",
                    msg
                );
            }
        }
    }
}

impl<T: LightningMessage> BroadcastEventInterface<T> for Event<T> {
    fn originator(&self) -> NodeIndex {
        self.originator
    }

    fn take(&mut self) -> Option<T> {
        self.message.take()
    }

    fn propagate(self) {
        let _ = self.command_sender.send(Command::Propagate(self.digest));
    }

    fn mark_invalid_sender(self) {
        let _ = self
            .command_sender
            .send(Command::MarkInvalidSender(self.digest));
    }

    fn get_digest(&self) -> Digest {
        self.digest
    }
}
