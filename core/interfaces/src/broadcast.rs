use anyhow::Result;
use async_trait::async_trait;
use infusion::c;
use lightning_schema::LightningMessage;
use lightning_types::NodeIndex;

use crate::infu_collection::Collection;
use crate::signer::SignerInterface;
use crate::topology::TopologyInterface;
use crate::types::Topic;
use crate::{
    ApplicationInterface,
    ConfigConsumer,
    ConfigProviderInterface,
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
        app: ::ApplicationInterface,
        topology: ::TopologyInterface,
        signer: ::SignerInterface,
        notifier: ::NotifierInterface,
    ) {
        Self::init(
            config.get::<Self>(),
            app.sync_query(),
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
        sqr: c!(C::ApplicationInterface::SyncExecutor),
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
    type Event: BroadcastEventInterface<T> = infusion::Blank<T>;

    /// Publish a message.
    async fn send(&self, msg: &T);

    /// Await the next message in the topic, should only return `None` if there are
    /// no longer any new messages coming. (indicating that the gossip instance is
    /// shutdown.)
    async fn recv(&mut self) -> Option<T>;

    /// Receive a message with advanced functionality.
    async fn recv_event(&mut self) -> Option<Self::Event>;
}

#[infusion::blank]
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
}
