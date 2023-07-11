use std::{marker::PhantomData, sync::Arc};

use anyhow::Result;
use async_trait::async_trait;
use draco_interfaces::{
    signer::SignerInterface, ConfigConsumer, GossipInterface, GossipSubscriberInterface,
    NotifierInterface, TopologyInterface, WithStartAndShutdown,
};
use serde::de::DeserializeOwned;

use super::config::Config;

pub struct Gossip<S: SignerInterface, Topo: TopologyInterface, N: NotifierInterface> {
    signer: PhantomData<S>,
    topology: PhantomData<Topo>,
    notifier: PhantomData<N>,
}

#[async_trait]
impl<S: SignerInterface, Topo: TopologyInterface, N: NotifierInterface + Send + Sync>
    WithStartAndShutdown for Gossip<S, Topo, N>
{
    /// Returns true if this system is running or not.
    fn is_running(&self) -> bool {
        todo!()
    }

    /// Start the system, should not do anything if the system is already
    /// started.
    async fn start(&self) {
        todo!()
    }

    /// Send the shutdown signal to the system.
    async fn shutdown(&self) {
        todo!()
    }
}

#[async_trait]
impl<S: SignerInterface, Topo: TopologyInterface, N: NotifierInterface + Send + Sync>
    GossipInterface for Gossip<S, Topo, N>
{
    type Signer = S;

    type Topology = Topo;

    type Notifier = N;

    type Subscriber<T: DeserializeOwned + Send + Sync> = GossipSubscriber<T>;

    async fn init(
        _config: Self::Config,
        _topology: Arc<Self::Topology>,
        _signer: &Self::Signer,
    ) -> Result<Self> {
        Ok(Self {
            signer: PhantomData,
            topology: PhantomData,
            notifier: PhantomData,
        })
    }

    fn subscribe<T>(&self, _topic: draco_interfaces::Topic) -> Self::Subscriber<T>
    where
        T: DeserializeOwned + Send + Sync,
    {
        todo!()
    }

    fn broadcast_socket(&self) -> affair::Socket<draco_interfaces::GossipMessage, ()> {
        todo!()
    }
}

impl<S: SignerInterface, Topo: TopologyInterface, N: NotifierInterface> ConfigConsumer
    for Gossip<S, Topo, N>
{
    type Config = Config;

    const KEY: &'static str = "GOSSIP";
}

pub struct GossipSubscriber<T>(PhantomData<T>);

#[async_trait]
impl<T> GossipSubscriberInterface<T> for GossipSubscriber<T>
where
    T: DeserializeOwned + Send + Sync,
{
    async fn recv(&mut self) -> Option<T> {
        todo!()
    }
}
