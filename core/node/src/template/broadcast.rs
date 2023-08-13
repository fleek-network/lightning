use std::{marker::PhantomData, sync::Arc};

use anyhow::Result;
use async_trait::async_trait;
use lightning_interfaces::{
    schema::LightningMessage, signer::SignerInterface, BroadcastInterface, ConfigConsumer,
    ListenerConnector, NotifierInterface, PubSub, SyncQueryRunnerInterface, Topic,
    TopologyInterface, WithStartAndShutdown,
};

use super::{config::Config, pool::ConnectionPool};

pub struct Broadcast<
    Q: SyncQueryRunnerInterface,
    S: SignerInterface,
    Topo: TopologyInterface,
    N: NotifierInterface,
> {
    query: PhantomData<Q>,
    signer: PhantomData<S>,
    topology: PhantomData<Topo>,
    notifier: PhantomData<N>,
}

#[async_trait]
impl<
    Q: SyncQueryRunnerInterface,
    S: SignerInterface,
    Topo: TopologyInterface,
    N: NotifierInterface + Send + Sync,
> WithStartAndShutdown for Broadcast<Q, S, Topo, N>
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
impl<
    Q: SyncQueryRunnerInterface,
    S: SignerInterface,
    Topo: TopologyInterface,
    N: NotifierInterface + Send + Sync,
> BroadcastInterface for Broadcast<Q, S, Topo, N>
{
    type Signer = S;

    type Topology = Topo;

    type Notifier = N;

    type PubSub<T: Clone + LightningMessage> = MockPubSub<T>;

    type ConnectionPool = ConnectionPool<Q, S>;
    type Message = ();

    /// Initialize the gossip system with the config and the topology object..
    fn init(
        _config: Self::Config,
        _listener_connector: ListenerConnector<Self::ConnectionPool, Self::Message>,
        _topology: Arc<Self::Topology>,
        _signer: &Self::Signer,
        _notify: Self::Notifier,
    ) -> Result<Self> {
        Ok(Self {
            query: PhantomData,
            signer: PhantomData,
            topology: PhantomData,
            notifier: PhantomData,
        })
    }

    fn get_pubsub<T: LightningMessage + Clone>(&self, _topic: Topic) -> Self::PubSub<T> {
        MockPubSub(PhantomData::<T>)
    }
}

impl<Q: SyncQueryRunnerInterface, S: SignerInterface, Topo: TopologyInterface, N: NotifierInterface>
    ConfigConsumer for Broadcast<Q, S, Topo, N>
{
    type Config = Config;

    const KEY: &'static str = "GOSSIP";
}

#[derive(Clone)]
pub struct MockPubSub<T>(PhantomData<T>);

#[async_trait]
impl<T> PubSub<T> for MockPubSub<T>
where
    T: Clone + LightningMessage,
{
    async fn send(&self, _msg: &T) {}
    async fn recv(&mut self) -> Option<T> {
        None
    }
}
