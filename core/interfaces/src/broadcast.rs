use std::sync::Arc;

use anyhow::Result;
use async_trait::async_trait;
use serde::{de::DeserializeOwned, Serialize};

use crate::{
    signer::SignerInterface, topology::TopologyInterface, ConfigConsumer, NotifierInterface,
    WithStartAndShutdown,
};

/// Numerical value for different gossip topics used by Fleek Network.
// New topics can be added as the system grows.
pub enum Topic {
    /// The gossip topic for
    Consensus,
    /// The gossip topic for Fleek Network's indexer DHT.
    DistributedHashTable,
}
/// The gossip system in Fleek Network implements the functionality of broadcasting
/// messages to the rest of the nodes in the network.
#[async_trait]
pub trait BroadcastInterface: WithStartAndShutdown + ConfigConsumer + Sized + Send + Sync {
    /// The implementation of the topology algorithm in use.
    type Topology: TopologyInterface;

    /// The notifier that allows us to refresh the connections once the epoch changes.
    type Notifier: NotifierInterface;

    /// The signer that we can used to sign and submit messages.
    type Signer: SignerInterface;

    type PubSub<T: Serialize + DeserializeOwned + Send + Sync + Clone>: PubSub<T>;

    /// Initialize the gossip system with the config and the topology object..
    async fn init(
        config: Self::Config,
        topology: Arc<Self::Topology>,
        signer: &Self::Signer,
    ) -> Result<Self>;

    ///
    fn get_pubsub<T: Serialize + DeserializeOwned + Send + Sync + Clone>(
        &self,
        topic: Topic,
    ) -> Self::PubSub<T>;
}

#[async_trait]
pub trait PubSub<T: Serialize + DeserializeOwned + Send + Sync + Clone>:
    Clone + Send + Sync
{
    /// Publish a message.
    fn send(&self, msg: &T);

    /// Await the next message in the topic, should only return `None` if there are
    /// no longer any new messages coming. (indicating that the gossip instance is
    /// shutdown.)
    async fn recv(&mut self) -> Option<T>;
}
