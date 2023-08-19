use anyhow::Result;
use async_trait::async_trait;
use infusion::c;
use lightning_schema::LightningMessage;

use crate::infu_collection::Collection;
use crate::signer::SignerInterface;
use crate::topology::TopologyInterface;
use crate::types::Topic;
use crate::{
    ConfigConsumer,
    ConfigProviderInterface,
    ConnectionPoolInterface,
    ListenerConnector,
    NotifierInterface,
    WithStartAndShutdown,
};

/// The gossip system in Fleek Network implements the functionality of broadcasting
/// messages to the rest of the nodes in the network.
#[infusion::service]
pub trait BroadcastInterface<C: Collection>:
    WithStartAndShutdown + ConfigConsumer + Sized + Send + Sync
{
    fn _init(
        config: ::ConfigProviderInterface,
        pool: ::ConnectionPoolInterface,
        topology: ::TopologyInterface,
        signer: ::SignerInterface,
        notifier: ::NotifierInterface,
    ) {
        Self::init(
            config.get::<Self>(),
            pool.bind(crate::types::ServiceScope::Broadcast),
            topology.clone(),
            signer,
            notifier.clone(),
        )
    }

    /// The message type to be encoded/decoded for networking.
    type Message: LightningMessage;

    /// Pubsub topic for sending and receiving messages on a topic
    type PubSub<T: LightningMessage + Clone>: PubSub<T> = infusion::Blank<T>;

    /// Initialize the gossip system with the config and the topology object..
    fn init(
        config: Self::Config,
        listener_connector: ListenerConnector<C, c![C::ConnectionPoolInterface], Self::Message>,
        topology: c!(C::TopologyInterface),
        signer: &c!(C::SignerInterface),
        notifier: c!(C::NotifierInterface),
    ) -> Result<Self>;

    /// Get a send and receiver for messages in a pub-sub topic.
    fn get_pubsub<T: LightningMessage + Clone>(&self, topic: Topic) -> Self::PubSub<T>;
}

#[async_trait]
#[infusion::blank]
pub trait PubSub<T: LightningMessage + Clone>: Clone + Send + Sync {
    /// Publish a message.
    async fn send(&self, msg: &T);

    /// Await the next message in the topic, should only return `None` if there are
    /// no longer any new messages coming. (indicating that the gossip instance is
    /// shutdown.)
    async fn recv(&mut self) -> Option<T>;
}
