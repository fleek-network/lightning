

use anyhow::Result;
use async_trait::async_trait;
use infusion::infu;
use lightning_schema::LightningMessage;

use crate::{
    infu_collection::Collection, signer::SignerInterface, topology::TopologyInterface,
    types::Topic, ConfigConsumer, ConnectionPoolInterface, ListenerConnector,
    NotifierInterface, WithStartAndShutdown,
};

/// The gossip system in Fleek Network implements the functionality of broadcasting
/// messages to the rest of the nodes in the network.
#[async_trait]
pub trait BroadcastInterface: WithStartAndShutdown + ConfigConsumer + Sized + Send + Sync {
    infu!(BroadcastInterface @ Input);

    // -- DYNAMIC TYPES

    /// The implementation of the topology algorithm in use.
    type Topology: TopologyInterface;

    /// The notifier that allows us to refresh the connections once the epoch changes.
    type Notifier: NotifierInterface;

    /// The signer that we can used to sign and submit messages.
    type Signer: SignerInterface;

    /// The networking connection pool.
    type ConnectionPool: ConnectionPoolInterface;

    // -- BOUNDED TYPES

    /// The message type to be encoded/decoded for networking.
    type Message: LightningMessage;

    /// Pubsub topic for sending and receiving messages on a topic
    type PubSub<T: LightningMessage + Clone>: PubSub<T>;

    /// Initialize the gossip system with the config and the topology object..
    fn init(
        config: Self::Config,
        listener_connector: ListenerConnector<Self::ConnectionPool, Self::Message>,
        topology: Self::Topology,
        signer: &Self::Signer,
        notifier: Self::Notifier,
    ) -> Result<Self>;

    /// Get a send and receiver for messages in a pub-sub topic.
    fn get_pubsub<T: LightningMessage + Clone>(&self, topic: Topic) -> Self::PubSub<T>;
}

#[async_trait]
pub trait PubSub<T: LightningMessage + Clone>: Clone + Send + Sync {
    /// Publish a message.
    async fn send(&self, msg: &T);

    /// Await the next message in the topic, should only return `None` if there are
    /// no longer any new messages coming. (indicating that the gossip instance is
    /// shutdown.)
    async fn recv(&mut self) -> Option<T>;
}
