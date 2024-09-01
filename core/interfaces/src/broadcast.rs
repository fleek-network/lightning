use std::collections::HashSet;

use anyhow::Result;
use fdi::BuildGraph;
use lightning_schema::LightningMessage;
use lightning_types::{Digest, NodeIndex};

use crate::components::NodeComponents;
use crate::types::Topic;

/// The gossip system in Fleek Network implements the functionality of broadcasting
/// messages to the rest of the nodes in the network.
#[interfaces_proc::blank]
pub trait BroadcastInterface<C: NodeComponents>: BuildGraph + Sized + Send {
    /// The message type to be encoded/decoded for networking.
    #[blank(())]
    type Message: LightningMessage;

    /// Pubsub topic for sending and receiving messages on a topic
    type PubSub<T: LightningMessage + Clone>: PubSub<T>;

    /// Get a send and receiver for messages in a pub-sub topic.
    #[blank = Default::default()]
    fn get_pubsub<T: LightningMessage + Clone>(&self, topic: Topic) -> Self::PubSub<T>;
}

#[interfaces_proc::blank]
pub trait PubSub<T: LightningMessage + Clone>: Clone + Send + Sync {
    type Event: BroadcastEventInterface<T>;

    /// Publish a message. If `filter` is `Some(set)`, then the message
    /// will only be sent to nodes in `set`.
    async fn send(&self, msg: &T, filter: Option<HashSet<NodeIndex>>) -> Result<Digest>;

    /// Propagate a message that we already propagated before.
    async fn repropagate(&self, digest: Digest, filter: Option<HashSet<NodeIndex>>);

    /// Await the next message in the topic, should only return `None` if there are
    /// no longer any new messages coming. (indicating that the gossip instance is
    /// shutdown.)
    async fn recv(&mut self) -> Option<T>;

    /// Receive a message with advanced functionality.
    async fn recv_event(&mut self) -> Option<Self::Event>;
}

#[interfaces_proc::blank]
pub trait BroadcastEventInterface<T: LightningMessage>: Send + Sync {
    /// Should return the originator of the message.
    fn originator(&self) -> NodeIndex;

    /// Take the message. This will always initially be filled with a message.
    fn take(&mut self) -> Option<T>;

    /// Propagate the message to other peers. Unless this function is called we
    /// should not advertise the message to other peers.
    fn propagate(self);

    /// This method should be called when the body of the message would suggest
    /// that the originator MUST have been another node.
    fn mark_invalid_sender(self);

    /// Returns the digest of the broadcast message associated with this event.
    fn get_digest(&self) -> Digest;
}
