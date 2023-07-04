use std::sync::Arc;

use affair::Socket;
use async_trait::async_trait;
use serde::de::DeserializeOwned;

use crate::{topology::TopologyInterface, ConfigConsumer, NotifierInterface, WithStartAndShutdown};

/// Numerical value for different gossip topics used by Fleek Network.
// New topics can be added as the system grows.
pub enum Topic {
    /// The gossip topic for
    Consensus,
    /// The gossip topic for Fleek Network's indexer DHT.
    DistributedHashTable,
}

/// A gossip message under a specific topic.
pub struct GossipMessage {
    /// The topic to send the message to.
    pub topic: Topic,
    /// The serialized bytes for this message.
    pub payload: Vec<u8>,
}

/// The gossip system in Fleek Network implements the functionality of broadcasting
/// messages to the rest of the nodes in the network.
#[async_trait]
pub trait GossipInterface: WithStartAndShutdown + ConfigConsumer + Sized + Send + Sync {
    /// The implementation of the topology algorithm in use.
    type Topology: TopologyInterface;

    /// The notifier that allows us to refresh the connections once the epoch changes.
    type Notifier: NotifierInterface;

    /// Subscriber implementation used for listening on a topic.
    type Subscriber<T: DeserializeOwned>: GossipSubscriber<T>;

    /// Initialize the gossip system with the config and the topology object..
    async fn init(config: Self::Config, topology: Arc<Self::Topology>) -> Self;

    /// Get a socket which can be used to broadcast a message globally under any topic.
    fn broadcast_socket(&self) -> Socket<GossipMessage, ()>;

    /// Subscribe to a specific topic and returns the subscriber. The messages that can
    /// be deserialized as `T` are returned to the listener.
    fn subscribe<T>(&self, topic: Topic) -> Self::Subscriber<T>
    where
        T: DeserializeOwned;
}

/// A subscriber for the incoming messages under a topic.
#[async_trait]
pub trait GossipSubscriber<T>
where
    T: DeserializeOwned,
{
    /// Await the next message in the topic, should only return `None` if there are
    /// no longer any new messages coming. (indicating that the gossip instance is
    /// shutdown.)
    async fn recv(&mut self) -> Option<T>;
}
