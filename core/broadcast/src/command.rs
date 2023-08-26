use std::sync::Arc;

use lightning_interfaces::types::{NodeIndex, Topic};
use tokio::sync::{mpsc, oneshot};

use crate::{Message, MessageInternedId};

/// A message that might be shared across threads. This is already validated
/// and is what we send to the pubsub.
#[derive(Debug, Clone)]
pub struct SharedMessage {
    pub id: MessageInternedId,
    pub origin: NodeIndex,
    pub payload: Arc<[u8]>,
}

/// A recv call from a pubsub.
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
pub struct SendCmd {
    pub topic: Topic,
    pub payload: Vec<u8>,
}

/// A command is what is sent from the other threads to the event loop.
pub enum Command {
    Recv(RecvCmd),
    Send(SendCmd),
    /// Send a command to the event loop to cause a propagation of a
    /// previously seen message.
    ///
    /// The broadcast does not advertise a message unless the subscribers
    /// decides to do so.
    Propagate(MessageInternedId),
    //// Mark that a message had an invalid sender. We are still interested
    /// in this message digest, but not from the given origin.
    MarkInvalidSender(MessageInternedId),
}

pub type CommandSender = mpsc::UnboundedSender<Command>;
pub type CommandReceiver = mpsc::UnboundedReceiver<Command>;
