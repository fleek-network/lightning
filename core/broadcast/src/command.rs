use std::collections::HashSet;
use std::sync::Arc;

use lightning_interfaces::types::{Digest, NodeIndex, Topic};
use tokio::sync::{mpsc, oneshot};

/// A message that might be shared across threads. This is already validated
/// and is what we send to the pubsub.
#[derive(Debug, Clone)]
pub struct SharedMessage {
    pub digest: Digest,
    pub origin: NodeIndex,
    pub payload: Arc<[u8]>,
}

/// A recv call from a pubsub.
#[derive(Debug)]
pub struct RecvCmd {
    /// The topic we are interested at.
    pub topic: Topic,
    /// The last message index in the ring buffer which this receiver has
    /// seen.
    pub last_seen: Option<usize>,
    /// The channel to send the response to.
    /// The number is the position of the element in the ring buffer.
    pub response: oneshot::Sender<(usize, SharedMessage)>,
}

/// A send call from a pubsub.
#[derive(Debug)]
pub struct SendCmd {
    pub topic: Topic,
    /// If `filter` is Some(set), then the message will only be send to the nodes in `set`
    pub filter: Option<HashSet<NodeIndex>>,
    pub payload: Vec<u8>,
    /// The response channel for returning the message digest.
    pub response: oneshot::Sender<Digest>,
}

/// A propagate call from a pubsub.
#[derive(Debug)]
pub struct PropagateCmd {
    pub digest: Digest,
    /// If `filter` is Some(set), then the message will only be send to the nodes in `set`
    pub filter: Option<HashSet<NodeIndex>>,
}

/// A command is what is sent from the other threads to the event loop.
#[derive(Debug)]
pub enum Command {
    /// Request to read a message from a topic, contains a oneshot which
    /// we use to send back the latest message.
    Recv(RecvCmd),
    /// Send a message under the given topic and with the given payload.
    Send(SendCmd),
    /// Send a command to the event loop to accept a message, without propagating it.
    ///
    /// The broadcast does not advertise a message unless the subscribers
    /// decides to do so.
    CleanUp(Digest),
    /// Send a command to the event loop to cause a propagation of a
    /// previously seen message.
    ///
    /// The broadcast does not advertise a message unless the subscribers
    /// decides to do so.
    Propagate(PropagateCmd),
    //// Mark that a message had an invalid sender. We are still interested
    /// in this message digest, but not from the given origin.
    MarkInvalidSender(Digest),
}

pub type CommandSender = mpsc::UnboundedSender<Command>;
pub type CommandReceiver = mpsc::UnboundedReceiver<Command>;
