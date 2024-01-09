use std::collections::HashSet;

use anyhow::Result;
use infusion::c;
use lightning_schema::LightningMessage;
use lightning_types::{Digest, NodeIndex};

use crate::infu_collection::Collection;
use crate::signer::SignerInterface;
use crate::types::Topic;
use crate::{
    ApplicationInterface,
    ConfigConsumer,
    ConfigProviderInterface,
    PoolInterface,
    ReputationAggregatorInterface,
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
        signer: ::SignerInterface,
        rep_aggregator: ::ReputationAggregatorInterface,
        pool: ::PoolInterface,
    ) {
        Self::init(
            config.get::<Self>(),
            app.sync_query(),
            signer,
            rep_aggregator.get_reporter(),
            pool,
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
        signer: &c!(C::SignerInterface),
        rep_reporter: c![C::ReputationAggregatorInterface::ReputationReporter],
        pool: &c!(C::PoolInterface),
    ) -> Result<Self>;

    /// Get a send and receiver for messages in a pub-sub topic.
    fn get_pubsub<T: LightningMessage + Clone>(&self, topic: Topic) -> Self::PubSub<T>;
}

#[infusion::blank]
pub trait PubSub<T: LightningMessage + Clone>: Clone + Send + Sync {
    type Event: BroadcastEventInterface<T> = infusion::Blank<T>;

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

    /// Returns the digest of the broadcast message associated with this event.
    fn get_digest(&self) -> Digest;
}
