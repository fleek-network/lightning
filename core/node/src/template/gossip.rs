use std::{marker::PhantomData, sync::Arc};

use anyhow::Result;
use async_trait::async_trait;
use freek_interfaces::{
    signer::SignerInterface, ConfigConsumer, GossipInterface, NotifierInterface, PubSub, Topic,
    TopologyInterface, WithStartAndShutdown,
};
use serde::{de::DeserializeOwned, Serialize};

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

    type PubSub<T: DeserializeOwned + Send + Sync + Clone + Serialize> = MockPubSub<T>;

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

    fn get_pubsub<T: Serialize + DeserializeOwned + Send + Sync + Clone>(
        &self,
        _topic: Topic,
    ) -> Self::PubSub<T> {
        MockPubSub(PhantomData::<T>)
    }
}

impl<S: SignerInterface, Topo: TopologyInterface, N: NotifierInterface> ConfigConsumer
    for Gossip<S, Topo, N>
{
    type Config = Config;

    const KEY: &'static str = "GOSSIP";
}

#[derive(Clone)]
pub struct MockPubSub<T>(PhantomData<T>);

#[async_trait]
impl<T> PubSub<T> for MockPubSub<T>
where
    T: DeserializeOwned + Send + Sync + Clone + Serialize,
{
    fn send(&self, _msg: &T) {}
    async fn recv(&mut self) -> Option<T> {
        None
    }
}
