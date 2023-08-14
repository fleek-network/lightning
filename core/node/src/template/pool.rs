use std::marker::PhantomData;

use async_trait::async_trait;
use lightning_interfaces::{
    schema::LightningMessage, types::ServiceScope, ConfigConsumer, ConnectionPoolInterface,
    ConnectorInterface, ListenerInterface, ReceiverInterface, SenderInterface, SenderReceiver,
    SignerInterface, SyncQueryRunnerInterface, WithStartAndShutdown,
};

use super::config::Config;

pub struct ConnectionPool<Q: SyncQueryRunnerInterface, S: SignerInterface> {
    _q: PhantomData<(Q, S)>,
}

#[derive(Debug)]
pub struct Connector<Q: SyncQueryRunnerInterface, S: SignerInterface, T> {
    _q: PhantomData<Q>,
    _x: PhantomData<(S, T)>,
}

impl<Q: SyncQueryRunnerInterface, S: SignerInterface, T> Clone for Connector<Q, S, T> {
    fn clone(&self) -> Self {
        Self {
            _q: self._q,
            _x: self._x,
        }
    }
}

pub struct Listener<Q: SyncQueryRunnerInterface, S: SignerInterface, T> {
    _q: PhantomData<Q>,
    _x: PhantomData<(S, T)>,
}

pub struct Receiver<T> {
    _x: PhantomData<T>,
}

pub struct Sender<T> {
    _x: PhantomData<T>,
}

impl<Q: SyncQueryRunnerInterface, S: SignerInterface> ConnectionPoolInterface
    for ConnectionPool<Q, S>
{
    type QueryRunner = Q;
    type Signer = S;

    type Connector<T: LightningMessage> = Connector<Q, S, T>;

    type Listener<T: LightningMessage> = Listener<Q, S, T>;

    /// The sender struct used across the sender and connector.
    type Sender<T: LightningMessage> = Sender<T>;

    /// The receiver struct used across the sender and connector.
    type Receiver<T: LightningMessage> = Receiver<T>;

    fn init(
        _config: Self::Config,
        _signer: &Self::Signer,
        _query_runner: Self::QueryRunner,
    ) -> Self {
        Self { _q: PhantomData }
    }

    fn bind<T>(&self, _scope: ServiceScope) -> (Self::Listener<T>, Self::Connector<T>)
    where
        T: LightningMessage,
    {
        (
            Listener {
                _q: PhantomData,
                _x: PhantomData,
            },
            Connector {
                _q: PhantomData,
                _x: PhantomData,
            },
        )
    }
}

impl<Q: SyncQueryRunnerInterface, S: SignerInterface> ConfigConsumer for ConnectionPool<Q, S> {
    const KEY: &'static str = "connection-pool";

    type Config = Config;
}

#[async_trait]
impl<Q: SyncQueryRunnerInterface, S: SignerInterface> WithStartAndShutdown
    for ConnectionPool<Q, S>
{
    /// Returns true if this system is running or not.
    fn is_running(&self) -> bool {
        true
    }

    /// Start the system, should not do anything if the system is already
    /// started.
    async fn start(&self) {}

    /// Send the shutdown signal to the system.
    async fn shutdown(&self) {}
}

#[async_trait]
impl<Q: SyncQueryRunnerInterface, S: SignerInterface, T: LightningMessage> ConnectorInterface<T>
    for Connector<Q, S, T>
{
    type ConnectionPool = ConnectionPool<Q, S>;

    async fn connect(
        &self,
        _to: &fleek_crypto::NodePublicKey,
    ) -> Option<lightning_interfaces::SenderReceiver<Self::ConnectionPool, T>> {
        None
    }
}

#[async_trait]
impl<Q: SyncQueryRunnerInterface, S: SignerInterface, T: LightningMessage> ListenerInterface<T>
    for Listener<Q, S, T>
{
    type ConnectionPool = ConnectionPool<Q, S>;

    async fn accept(&mut self) -> Option<SenderReceiver<Self::ConnectionPool, T>> {
        None
    }
}

#[async_trait]
impl<T: LightningMessage> SenderInterface<T> for Sender<T> {
    fn pk(&self) -> &fleek_crypto::NodePublicKey {
        &fleek_crypto::NodePublicKey([0; 96])
    }

    async fn send(&self, _msg: T) -> bool {
        false
    }
}

#[async_trait]
impl<T: LightningMessage> ReceiverInterface<T> for Receiver<T> {
    fn pk(&self) -> &fleek_crypto::NodePublicKey {
        &fleek_crypto::NodePublicKey([0; 96])
    }

    async fn recv(&mut self) -> Option<T> {
        None
    }
}
